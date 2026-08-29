/// Where this project lives, as the build read it out of Cargo.toml.
///
/// A page links back to the repository in three or four places; the address of
/// it belongs in the manifest that already declares it, not in each of them.
declare const __REPO__: string;

export const REPO = __REPO__;

/// A file in the repository, as the branch that is published shows it.
export const inRepo = (path: string) => `${REPO}/blob/main/${path}`;
