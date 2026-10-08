//! Private NSIS transaction helper. Settings and unknown installation files are never owned.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    os::windows::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};
use windows_registry::{CURRENT_USER, Type};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub const LEGACY_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\25f85616-d352-524a-a5cc-01558c0b783c";
pub const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Lumiere";
pub const PRODUCT_KEY: &str = r"Software\Lumiere\Lumiere";
const NEW_FILES: &[&str] = &[
    "Lumiere.exe",
    "lumiere-windows-host.exe",
    "uninstall.exe",
    ".lumiere-install.json",
];
#[derive(Serialize, Deserialize)]
struct RegistryValue {
    name: String,
    kind: u32,
    bytes: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
struct RegistrySnapshot {
    path: String,
    values: Option<Vec<RegistryValue>>,
}
#[derive(Serialize, Deserialize)]
struct FileSnapshot {
    path: PathBuf,
    backup: String,
    existed: bool,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    version: u8,
    root: PathBuf,
    owner: u32,
    owner_time: u64,
    committed: bool,
    files: Vec<FileSnapshot>,
    registry: Vec<RegistrySnapshot>,
}
#[derive(Deserialize)]
struct LegacyInventory {
    files: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub files: Vec<ManifestFile>,
}
#[derive(Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub sha256: String,
}

fn fail(message: &str) -> Box<dyn std::error::Error> {
    io::Error::other(message).into()
}
fn normal_root(root: &Path) -> Result<PathBuf> {
    if !root.is_absolute() || root.parent().is_none() || root.file_name().is_none() {
        return Err(fail(
            "Installation root must be an absolute non-root directory",
        ));
    }
    if root
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(fail("Installation root contains relative components"));
    }
    reject_reparse(root)?;
    Ok(root.to_path_buf())
}
fn reject_reparse(path: &Path) -> Result<()> {
    for parent in path.ancestors() {
        match fs::symlink_metadata(parent) {
            Ok(meta) if meta.file_attributes() & 0x400 != 0 => {
                return Err(fail(
                    "Reparse points are not allowed in owned installer paths",
                ));
            }
            Ok(_) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
fn relative_path(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(fail("Inventory path escapes the installation root"));
    }
    Ok(path)
}
fn transaction_dir(root: &Path) -> PathBuf {
    let digest = Sha256::digest(root.to_string_lossy().to_lowercase().as_bytes());
    root.parent()
        .unwrap()
        .join(format!(".lumiere-upgrade-{:x}", digest))
}
fn write_sync(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn replace(source: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::Storage::FileSystem::{
            MOVEFILE_COPY_ALLOWED, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        },
        core::PCWSTR,
    };
    let source: Vec<_> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<_> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: both nul-terminated paths remain live for the synchronous operation.
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH | MOVEFILE_COPY_ALLOWED,
        )?;
    }
    Ok(())
}
fn store_journal(directory: &Path, journal: &Journal) -> Result<()> {
    let temporary = directory.join("journal.tmp");
    write_sync(&temporary, &serde_json::to_vec(journal)?)?;
    replace(&temporary, &directory.join("journal.json"))
}
fn process_time(pid: u32) -> Option<u64> {
    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    if pid == 0 {
        return None;
    }
    // SAFETY: query-only handle, writable FILETIME storage, close on all paths.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let result = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        let _ = CloseHandle(handle);
        result.ok()?;
        if exit.dwHighDateTime != 0 || exit.dwLowDateTime != 0 {
            return None;
        }
        Some((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }
}
fn registry_snapshot(path: &str) -> Result<RegistrySnapshot> {
    let values = match CURRENT_USER.open(path) {
        Ok(key) => {
            if key.keys()?.next().is_some() {
                return Err(fail("Unexpected subkeys in owned installer registration"));
            }
            Some(
                key.values()?
                    .map(|(name, value)| RegistryValue {
                        name,
                        kind: value.ty().into(),
                        bytes: value.to_vec(),
                    })
                    .collect(),
            )
        }
        Err(error) if error.code().0 as u32 == 0x80070002 => None,
        Err(error) => return Err(error.into()),
    };
    Ok(RegistrySnapshot {
        path: path.into(),
        values,
    })
}
fn restore_registry(snapshot: &RegistrySnapshot) -> Result<()> {
    match CURRENT_USER.remove_tree(&snapshot.path) {
        Ok(()) => (),
        Err(error) if error.code().0 as u32 == 0x80070002 => (),
        Err(error) => return Err(error.into()),
    }
    if let Some(values) = &snapshot.values {
        let key = CURRENT_USER.create(&snapshot.path)?;
        for value in values {
            key.set_bytes(&value.name, Type::from(value.kind), &value.bytes)?;
        }
    }
    Ok(())
}
fn registered_root(key: &str) -> Option<PathBuf> {
    let key = CURRENT_USER.open(key).ok()?;
    if let Ok(location) = key.get_string("InstallLocation")
        && !location.is_empty()
    {
        return Some(PathBuf::from(location.trim_matches('"')));
    }
    let command = key.get_string("UninstallString").ok()?;
    // Parse the known quoted executable, never execute or interpret a shell command.
    let executable = command.strip_prefix('"')?.split('"').next()?;
    Path::new(executable).parent().map(Path::to_path_buf)
}
fn same_root(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .trim_end_matches('\\')
        .eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches('\\'))
}
pub fn inherited_root() -> Option<PathBuf> {
    registered_root(UNINSTALL_KEY).or_else(|| registered_root(LEGACY_KEY))
}
fn owned_paths(root: &Path) -> Result<BTreeSet<PathBuf>> {
    let inventory: LegacyInventory = serde_json::from_str(include_str!(
        "../../../apps/desktop/build/windows-installer/legacy-files.v0.6.0.json"
    ))?;
    inventory
        .files
        .iter()
        .map(String::as_str)
        .chain(NEW_FILES.iter().copied())
        .chain(["Uninstall Lumiere.exe"])
        .map(|value| Ok(root.join(relative_path(value)?)))
        .collect()
}
fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
fn cleanup(directory: &Path, root: &Path) -> Result<()> {
    // Only this exact hashed sibling may be recursively deleted. It is outside
    // the installation root, has no user files, and each descendant is checked.
    if directory != transaction_dir(root) || directory.starts_with(root) {
        return Err(fail("Invalid backup directory"));
    }
    fn check_tree(path: &Path) -> Result<()> {
        reject_reparse(path)?;
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                check_tree(&entry?.path())?;
            }
        }
        Ok(())
    }
    check_tree(directory)?;
    fs::remove_dir_all(directory)?;
    Ok(())
}
pub fn begin(root: &Path, owner: u32, shortcuts: &[PathBuf]) -> Result<()> {
    let previous = inherited_root();
    let mut registrations = vec![UNINSTALL_KEY, PRODUCT_KEY];
    if registered_root(LEGACY_KEY).is_some_and(|previous| same_root(&previous, root)) {
        registrations.push(LEGACY_KEY);
    }
    begin_inner(root, owner, shortcuts, previous.as_deref(), &registrations)
}
fn begin_inner(
    root: &Path,
    owner: u32,
    shortcuts: &[PathBuf],
    previous: Option<&Path>,
    registrations: &[&str],
) -> Result<()> {
    let root = normal_root(root)?;
    let directory = transaction_dir(&root);
    if directory.exists() {
        let journal_path = directory.join("journal.json");
        if journal_path.exists() {
            let journal: Journal = serde_json::from_slice(&fs::read(&journal_path)?)?;
            if !same_root(&journal.root, &root) || journal.version != 1 {
                return Err(fail("Unknown installation journal"));
            }
            if process_time(journal.owner).is_some_and(|time| time == journal.owner_time) {
                return Err(fail("Another installer is active"));
            }
            rollback(&root)?;
        } else {
            // A crash while copying backups cannot have changed product files.
            // An exclusive preparation marker prevents a concurrent begin.
            let marker: (PathBuf, u32, u64) =
                serde_json::from_slice(&fs::read(directory.join("preparing.json"))?)?;
            if !same_root(&marker.0, &root)
                || process_time(marker.1).is_some_and(|time| time == marker.2)
            {
                return Err(fail("Another installer is preparing backups"));
            }
            cleanup(&directory, &root)?;
        }
    }
    // A registered installation keeps its location. /D cannot silently strand it.
    if let Some(previous) = previous
        && !same_root(previous, &root)
    {
        return Err(fail("Use the existing Lumiere installation location"));
    }
    fs::create_dir(&directory)?;
    let owner_time = process_time(owner).unwrap_or(0);
    write_sync(
        &directory.join("preparing.json"),
        &serde_json::to_vec(&(root.clone(), owner, owner_time))?,
    )?;
    let result = (|| {
        let mut paths = owned_paths(&root)?;
        for shortcut in shortcuts {
            if !shortcut.is_absolute()
                || shortcut
                    .file_name()
                    .is_none_or(|name| name != "Lumiere.lnk")
            {
                return Err(fail("Invalid shortcut path"));
            }
            paths.insert(shortcut.clone());
        }
        let mut journal = Journal {
            version: 1,
            root: root.clone(),
            owner,
            owner_time,
            committed: false,
            files: vec![],
            registry: vec![],
        };
        for (index, path) in paths.into_iter().enumerate() {
            reject_reparse(&path)?;
            let existed = path.try_exists()?;
            let backup = format!("file-{index}");
            if existed {
                if !path.is_file() {
                    return Err(fail("An owned program path is not a file"));
                }
                fs::copy(&path, directory.join(&backup))?;
                fs::OpenOptions::new()
                    .write(true)
                    .open(directory.join(&backup))?
                    .sync_all()?;
            }
            journal.files.push(FileSnapshot {
                path,
                backup,
                existed,
            });
        }
        for key in registrations {
            journal.registry.push(registry_snapshot(key)?);
        }
        store_journal(&directory, &journal)
    })();
    if result.is_err() {
        cleanup(&directory, &root)?;
    }
    result
}
fn load(root: &Path) -> Result<(PathBuf, Journal)> {
    let root = normal_root(root)?;
    let directory = transaction_dir(&root);
    reject_reparse(&directory)?;
    let journal: Journal = serde_json::from_slice(&fs::read(directory.join("journal.json"))?)?;
    if journal.version != 1 || !same_root(&journal.root, &root) {
        return Err(fail("Unknown installation journal"));
    }
    Ok((directory, journal))
}
pub fn rollback(root: &Path) -> Result<()> {
    let (directory, journal) = load(root)?;
    if !journal.committed {
        for file in &journal.files {
            reject_reparse(&file.path)?;
            if file.existed {
                fs::create_dir_all(file.path.parent().unwrap())?;
                let temporary = directory.join("restore.tmp");
                fs::copy(directory.join(&file.backup), &temporary)?;
                replace(&temporary, &file.path)?;
            } else {
                remove_if_exists(&file.path)?;
            }
        }
        for registry in &journal.registry {
            restore_registry(registry)?;
        }
        remove_empty_owned_directories(root)?;
    }
    cleanup(&directory, root)
}
fn remove_empty_owned_directories(root: &Path) -> Result<()> {
    let mut directories = BTreeSet::new();
    for path in owned_paths(root)? {
        for parent in path.ancestors().skip(1).take_while(|p| *p != root) {
            directories.insert(parent.to_path_buf());
        }
    }
    let mut directories: Vec<_> = directories.into_iter().collect();
    directories.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for directory in directories {
        reject_reparse(&directory)?;
        match fs::remove_dir(directory) {
            Ok(()) => (),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub fn commit(root: &Path, manifest: &Manifest) -> Result<()> {
    let (directory, mut journal) = load(root)?;
    if journal.committed {
        return cleanup(&directory, root);
    }
    let names: BTreeSet<_> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    if names != BTreeSet::from(["Lumiere.exe", "lumiere-windows-host.exe"]) {
        return Err(fail("Unexpected installation payload"));
    }
    for file in &manifest.files {
        let path = root.join(relative_path(&file.path)?);
        reject_reparse(&path)?;
        if format!("{:x}", Sha256::digest(fs::read(path)?)) != file.sha256 {
            return Err(fail("Installed payload hash mismatch"));
        }
    }
    if journal
        .registry
        .iter()
        .any(|snapshot| snapshot.path == UNINSTALL_KEY)
    {
        let registration = CURRENT_USER.open(UNINSTALL_KEY)?;
        if !registered_root(UNINSTALL_KEY).is_some_and(|location| same_root(&location, root))
            || registration.get_string("DisplayVersion")? != manifest.version
        {
            return Err(fail("Installation registration was not written"));
        }
    }
    if !root.join("uninstall.exe").is_file() {
        return Err(fail("Uninstaller was not written"));
    }
    for path in owned_paths(root)? {
        if NEW_FILES.iter().any(|name| path == root.join(name)) {
            continue;
        }
        reject_reparse(&path)?;
        remove_if_exists(&path)?;
    }
    remove_empty_owned_directories(root)?;
    if registered_root(LEGACY_KEY).is_some_and(|previous| same_root(&previous, root)) {
        CURRENT_USER.remove_tree(LEGACY_KEY)?;
    }
    write_sync(
        &directory.join("manifest.tmp"),
        &serde_json::to_vec(manifest)?,
    )?;
    replace(
        &directory.join("manifest.tmp"),
        &root.join(".lumiere-install.json"),
    )?;
    journal.committed = true;
    store_journal(&directory, &journal)?;
    cleanup(&directory, root)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        base: PathBuf,
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "lumiere-install-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let root = base.join("custom install");
            fs::create_dir_all(&root).unwrap();
            Self { base, root }
        }
        fn begin(&self) {
            begin_inner(&self.root, 0, &[], None, &[]).unwrap();
        }
        fn payload(&self) -> Manifest {
            let mut files = vec![];
            for name in ["Lumiere.exe", "lumiere-windows-host.exe"] {
                fs::write(self.root.join(name), format!("new-{name}")).unwrap();
                files.push(ManifestFile {
                    path: name.into(),
                    sha256: format!(
                        "{:x}",
                        Sha256::digest(fs::read(self.root.join(name)).unwrap())
                    ),
                });
            }
            fs::write(self.root.join("uninstall.exe"), b"new-uninstaller").unwrap();
            Manifest {
                version: "0.6.0".into(),
                files,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert!(
                self.base.starts_with(std::env::temp_dir()) && self.root.starts_with(&self.base)
            );
            fs::remove_dir_all(&self.base).unwrap();
        }
    }
    #[test]
    fn fresh_commit_and_next_tauri_rollback_preserve_unknown_files() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("user notes.txt"), b"owned by user").unwrap();
        fixture.begin();
        let payload = fixture.payload();
        commit(&fixture.root, &payload).unwrap();
        assert!(fixture.root.join(".lumiere-install.json").is_file());
        fixture.begin();
        fs::write(fixture.root.join("Lumiere.exe"), b"partial newer version").unwrap();
        rollback(&fixture.root).unwrap();
        assert_eq!(
            fs::read(fixture.root.join("Lumiere.exe")).unwrap(),
            b"new-Lumiere.exe"
        );
        assert_eq!(
            fs::read(fixture.root.join("user notes.txt")).unwrap(),
            b"owned by user"
        );
    }
    #[test]
    fn legacy_commit_cleans_only_inventory_and_does_not_execute_uninstaller() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.root.join("resources")).unwrap();
        fs::write(fixture.root.join("resources/app.asar"), b"legacy-app").unwrap();
        fs::write(fixture.root.join("resources/user.png"), b"unknown image").unwrap();
        fs::write(
            fixture.root.join("Uninstall Lumiere.exe"),
            b"must never execute",
        )
        .unwrap();
        fixture.begin();
        let payload = fixture.payload();
        commit(&fixture.root, &payload).unwrap();
        assert!(!fixture.root.join("resources/app.asar").exists());
        assert!(!fixture.root.join("Uninstall Lumiere.exe").exists());
        assert_eq!(
            fs::read(fixture.root.join("resources/user.png")).unwrap(),
            b"unknown image"
        );
    }
    #[test]
    fn interrupted_replacement_restores_then_starts_a_new_transaction() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("Lumiere.exe"), b"old executable").unwrap();
        fs::write(fixture.root.join("chrome_100_percent.pak"), b"old resource").unwrap();
        fixture.begin();
        fixture.payload();
        fs::remove_file(fixture.root.join("chrome_100_percent.pak")).unwrap();
        fixture.begin();
        assert_eq!(
            fs::read(fixture.root.join("Lumiere.exe")).unwrap(),
            b"old executable"
        );
        assert_eq!(
            fs::read(fixture.root.join("chrome_100_percent.pak")).unwrap(),
            b"old resource"
        );
        assert!(!fixture.root.join("lumiere-windows-host.exe").exists());
        rollback(&fixture.root).unwrap();
    }
    #[test]
    fn hash_failure_retains_rollback_and_corrupt_settings_are_untouched() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("settings.json"), b"{corrupt user file").unwrap();
        fixture.begin();
        let payload = fixture.payload();
        fs::write(fixture.root.join("Lumiere.exe"), b"corrupted payload").unwrap();
        assert!(commit(&fixture.root, &payload).is_err());
        rollback(&fixture.root).unwrap();
        assert!(!fixture.root.join("Lumiere.exe").exists());
        assert_eq!(
            fs::read(fixture.root.join("settings.json")).unwrap(),
            b"{corrupt user file"
        );
    }
    #[test]
    fn rejects_traversal_root_and_live_concurrent_installer() {
        assert!(normal_root(Path::new("C:\\")).is_err());
        assert!(relative_path("../user.txt").is_err());
        let fixture = Fixture::new();
        begin_inner(&fixture.root, std::process::id(), &[], None, &[]).unwrap();
        assert!(begin_inner(&fixture.root, 0, &[], None, &[]).is_err());
        rollback(&fixture.root).unwrap();
    }
    #[test]
    fn registration_snapshot_round_trip_keeps_raw_values_and_absence() {
        let path = format!(r"Software\LumiereInstallerTest\{}", std::process::id());
        let absent = registry_snapshot(&path).unwrap();
        let key = CURRENT_USER.create(&path).unwrap();
        key.set_string("", "custom path").unwrap();
        key.set_u32("flag", 42).unwrap();
        let original = registry_snapshot(&path).unwrap();
        key.set_string("", "new path").unwrap();
        key.set_string("added", "new value").unwrap();
        restore_registry(&original).unwrap();
        let restored = CURRENT_USER.open(&path).unwrap();
        assert_eq!(restored.get_string("").unwrap(), "custom path");
        assert_eq!(restored.get_u32("flag").unwrap(), 42);
        assert!(restored.get_string("added").is_err());
        drop(restored);
        drop(key);
        restore_registry(&absent).unwrap();
        assert!(CURRENT_USER.open(&path).is_err());
    }
}
