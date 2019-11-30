use std::fmt;

/// An inline string used as a compact, human-readable object
/// identifier. Strikes a compromise between the efficiency of string
/// interning and the ease of using strings.
// TODO: If desired, use small string trick to guarantee final null.
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd)]
pub struct Name {
    // N.B. all unused bytes must be 0 for hashing
    bytes: [u8; Name::capacity()],
    len: u8,
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl Default for Name {
    fn default() -> Self {
        Name::empty()
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        let slice = &self.bytes[..self.len()];
        unsafe { std::str::from_utf8_unchecked(slice) }
    }
}

impl AsRef<[u8]> for Name {
    fn as_ref(&self) -> &[u8] {
        &self.bytes[..self.len()]
    }
}

impl Name {
    pub const fn capacity() -> usize {
        23
    }

    pub fn len(&self) -> usize {
        self.len as _
    }

    pub fn empty() -> Self {
        Name {
            bytes: [0; Name::capacity()],
            len: 0,
        }
    }

    /// Creates a new name from a string. The string must be at most
    /// `Name::capacity()` bytes.
    pub fn new(src: &str) -> Self {
        let src = src.as_bytes();
        assert!(
            src.len() <= Name::capacity(),
            "len {} > {}", src.len(), Name::capacity(),
        );
        let mut bytes = [0; Name::capacity()];
        bytes[..src.len()].copy_from_slice(src);
        Name { bytes, len: src.len() as _ }
    }

    /// Creates a name from a string, truncating the string to at most
    /// `Name::capacity()` bytes. Truncation always occurs at a
    /// codepoint boundary. May split multi-codepoint characters, such
    /// as characters made of combining diacritics.
    pub fn new_trunc(src: &str) -> Self {
        if src.len() <= Name::capacity() {
            return Name::new(src);
        }

        // Find a char boundary
        let len = (0..Name::capacity() + 1)
            .rev()
            .find(|&i| src.is_char_boundary(i))
            .unwrap();
        let src = src.as_bytes();
        let src = &src[..len];
        let mut bytes = [0; Name::capacity()];
        bytes[..len].copy_from_slice(src);
        Name { bytes, len: len as _ }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }

    pub fn as_str(&self) -> &str {
        self.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_test() {
        let name = Name::new("oh boy");
        assert_eq!(name.as_str(), "oh boy");
        assert_eq!(name.as_bytes(), b"oh boy");

        let name = Name::new_trunc("abcdefghijklmnopqrstuvwxyz");
        assert_eq!(name.as_str(), "abcdefghijklmnopqrstuvw");

        assert_eq!(Name::empty().as_str(), "");
    }

    #[test]
    #[should_panic]
    fn truncate_test() {
        Name::new("abcdefghijklmnopqrstuvwxyz");
    }

    #[test]
    fn unicode_test() {
        assert_eq!(Name::new("😂").as_str(), "😂");
        // The final emoji is 4 bytes and so gets truncated
        assert_eq!(Name::new_trunc("😂😂😂😂😂😂").as_str(), "😂😂😂😂😂");
        // The final diacritic is truncated but not the base char
        assert_eq!(Name::new_trunc("ääảäääảä").as_str(), "ääảäääảa");
    }

    #[test]
    fn eq_test() {
        let x = Name::new("");
        let y = Name::new("12");
        let z = x;
        let w = Name::new("😂");
        assert_eq!(x, x);
        assert_eq!(x, Name::new(""));
        assert_eq!(x, z);
        assert_eq!(y, y);
        assert_eq!(w, Name::new("😂"));
    }

    #[test]
    fn hash_test() {
        use std::collections::HashSet;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        let mut hasher = DefaultHasher::new();

        let x = Name::new("123");
        assert_eq!(x.hash(&mut hasher), x.hash(&mut hasher));

        let set: HashSet<_> = vec![
            x,
            Default::default(),
            Name::new("😂"),
            Name::new("123"),
            Name::empty(),
        ].into_iter().collect();

        assert_eq!(set.len(), 3);
        assert!(set.contains(&Name::new("123")));
        assert!(set.contains(&Name::new("😂")));
        assert!(set.contains(&Name::new("")));
    }

    #[test]
    fn display_test() {
        let strings = [
            "",
            "foo",
            "bar",
            "😂",
            "中国",
        ];
        for string in strings.iter() {
            assert_eq!(&Name::new(string).to_string(), string);
        }
    }
}
