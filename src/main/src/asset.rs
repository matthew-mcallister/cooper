/// This module controls how assets are loaded.
///
/// Assets come from bundles, such as a filesystem directory or ZIP
/// archive. Each asset is given a name, usually its path within the
/// bundle it came from. Assets share a single, global namespace. If
/// multiple bundles have an asset with the same name, the bundle loaded
/// last will override previous bundles. That is, additional loaded
/// bundles will "patch" the base bundle.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use common::*;
use derive_more::*;

use crate::*;

/// TODO: This should either be or pretend to be an interned string
/// (i.e. a plain u32).
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct AssetName {
    inner: Arc<String>,
}

impl AssetName {
    pub fn new(value: String) -> Self {
        AssetName { inner: Arc::new(value) }
    }
}

impl std::ops::Deref for AssetName {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl std::borrow::Borrow<str> for AssetName {
    fn borrow(&self) -> &str {
        (*self.inner).borrow()
    }
}

/// A bundle is an object that contains a list of assets and knows how
/// to read them.
pub trait AssetBundle: std::fmt::Debug {
    fn assets(&self) -> Box<dyn Iterator<Item = &AssetName> + '_>;
    fn open(&self, index: u32) -> io::Result<Box<dyn io::Read>>;
}

/// Tells the asset manager where to find an asset, i.e. the bundle and
/// location within the bundle.
#[derive(Clone, Copy, Constructor, Debug, Eq, PartialEq)]
struct AssetSource {
    bundle: u32,
    index: u32,
}

#[derive(Debug)]
struct AssetState {
    source: AssetSource,
}

/// Looks up assets by name.
#[derive(Debug)]
pub struct AssetManager {
    assets: fnv::FnvHashMap<AssetName, AssetState>,
    bundles: Vec<Box<dyn AssetBundle>>,
}

impl AssetState {
    fn new(source: AssetSource) -> Self {
        AssetState {
            source,
        }
    }
}

impl AssetManager {
    pub fn new() -> Self {
        AssetManager {
            assets: Default::default(),
            bundles: Default::default(),
        }
    }

    pub fn add_bundle(&mut self, bundle: Box<dyn AssetBundle>) {
        let bindex = self.bundles.len() as u32;
        for (index, name) in bundle.assets().enumerate() {
            let source = AssetSource::new(bindex, index as u32);
            self.assets.insert(name.clone(), AssetState::new(source));
        }
        self.bundles.push(bundle);
    }

    /// Returns `None` if there is no asset with the given name.
    pub fn open(&self, name: &str) -> Option<io::Result<Box<dyn io::Read>>>
    {
        let src = self.assets.get(name)?.source;
        Some(self.bundles[src.bundle as usize].open(src.index))
    }
}

/// A bundle that pulls from a filesystem directory.
#[derive(Debug)]
pub struct DirectoryAssetBundle {
    base: PathBuf,
    assets: Vec<AssetName>,
}

impl DirectoryAssetBundle {
    pub fn new<P: Into<PathBuf>>(path: P) -> io::Result<Self> {
        use walkdir::WalkDir;
        let base = path.into();
        let mut assets = Vec::new();
        let log = |x: &walkdir::Error| println!("{}", x);
        let iter = WalkDir::new(&base)
            .follow_links(true)
            .into_iter()
            .filter_map(|res| res.on_err(log).ok())
            // TODO: Does this skip symlinks?
            .filter(|entry| entry.file_type().is_file());
        for entry in iter {
            let path = entry.path().strip_prefix(&base).unwrap().to_owned();
            // FIXME: This is in no way cross-platform.
            let name = path.into_os_string().into_string().unwrap();
            assets.push(AssetName::new(name));
        }
        assert!(assets.len() <= u32::max_value() as _);
        Ok(DirectoryAssetBundle { base, assets })
    }
}

impl AssetBundle for DirectoryAssetBundle {
    fn assets(&self) -> Box<dyn Iterator<Item = &AssetName> + '_> {
        Box::new(self.assets.iter())
    }

    fn open(&self, index: u32) -> io::Result<Box<dyn io::Read>> {
        let rel: &Path = Path::new(&*self.assets[index as usize]);
        assert!(rel.is_relative());
        let path = self.base.join(rel);
        Ok(Box::new(fs::File::open(&path)?))
    }
}

#[cfg(test)]
mod dir_bundle_tests {
    use std::borrow::Borrow;
    use std::collections::{HashMap, HashSet};
    use super::*;

    const TEST_DIR: &'static str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/assets");

    #[test]
    fn test_cons() {
        // TODO: This probably will fail on windows due to the separator
        let exp: HashSet<_> = ["foo", "sub/bar", "sub/baz"].iter().collect();

        let bundle = DirectoryAssetBundle::new(TEST_DIR).unwrap();
        let assets: Vec<_> = bundle.assets().map(|s| s.borrow()).collect();
        assert_eq!(assets.len(), exp.len());
        let assets: HashSet<_> = assets.iter().collect();

        assert_eq!(&assets, &exp);
    }

    #[test]
    fn test_open() {
        let pairs = [
            ("foo", b"1\n"),
            ("sub/bar", b"2\n"),
            ("sub/baz", b"3\n"),
        ];

        let bundle = DirectoryAssetBundle::new(TEST_DIR).unwrap();
        let assets: HashMap<&str, u32> = bundle.assets().enumerate()
            .map(|(i, s)| (s.borrow(), i as u32)).collect();

        for &(name, exp) in pairs.iter() {
            let index = assets[name];
            let mut content = Vec::new();
            bundle.open(index).unwrap().read_to_end(&mut content).unwrap();
            assert_eq!(&content, exp);
        }
    }
}

// TODO: For ZipAssetBundle (one per thread if multiple threads)
//struct ZipEntryReader {
//    shared_file: Rc<RefCell<ZipDecoder<File>>>,
//}
