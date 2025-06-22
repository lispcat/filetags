pub mod cleaning;
pub mod filesystem;
pub mod symlinking;

#[derive(Clone, Debug)]
pub enum Action {
    MakeNecessaryDirs,
    CleanAll,
    SymlinkAll,
}
