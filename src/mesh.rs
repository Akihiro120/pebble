pub trait Mesh {
    type Info;

    fn path() -> Option<&'static str>;
    fn vertices() -> Option<Vec<Self::Info>>;
    fn indices() -> Option<Vec<u32>>;
}
