pub mod cleaning;
pub mod filesystem;
pub mod symlinking;

#[derive(Clone, Debug)]
pub enum Action {
    CleanDir(usize, usize),
    MakeNecessaryDirs,
    CleanAll,
    SymlinkAll,
}
