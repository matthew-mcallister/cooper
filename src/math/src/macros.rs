macro_rules! impl_inf_sup {
    () => {
        fn inf<I>(mut iter: I) -> Option<Self>
            where I: Iterator<Item = Self>
        {
            let first = iter.next()?;
            Some(iter.fold(first, |a, b| a.inf(b)))
        }

        fn sup<I>(mut iter: I) -> Option<Self>
            where I: Iterator<Item = Self>
        {
            let first = iter.next()?;
            Some(iter.fold(first, |a, b| a.sup(b)))
        }

        fn inf_sup<I>(mut iter: I) -> crate::InfSupResult<Self>
            where I: Iterator<Item = Self>
        {
            use crate::InfSupResult;
            let first = match iter.next() {
                Some(x) => x,
                None => return InfSupResult::Empty,
            };
            let second = match iter.next() {
                Some(x) => x,
                None => return InfSupResult::Singleton(first),
            };
            let (inf, sup) = (first.inf(second), first.sup(second));
            let (inf, sup) = iter.fold(
                (inf, sup),
                |(inf, sup), v| (inf.inf(v), sup.sup(v)),
            );
            InfSupResult::InfSup(inf, sup)
        }
    }
}
