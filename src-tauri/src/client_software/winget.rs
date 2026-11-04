#![cfg(windows)]
mod api {
    #![allow(dead_code, non_snake_case, non_upper_case_globals, non_camel_case_types, unused_imports, clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/winget.rs"));
}

use api::*;
use super::{identity, types::{Installation, Manager, Owner}};
use asb_core::contracts::AppKind;
use std::{path::PathBuf, time::{Duration, Instant}};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER, COINIT_MULTITHREADED};
use windows_core::{GUID, HSTRING, Interface};

pub struct Apartment;
impl Apartment {
    pub fn new() -> Result<Self, String> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().map_err(|e| e.to_string())?; }
        Ok(Self)
    }
}
impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }

fn create<T: Interface>(id: u128) -> windows_core::Result<T> {
    unsafe { CoCreateInstance(&GUID::from_u128(id), None, CLSCTX_LOCAL_SERVER) }
}

fn manager() -> windows_core::Result<PackageManager> { create(0xc53a4f16_787e_42a4_b304_29effb4bf597) }

fn find(reference: &PackageCatalogReference, id: &str) -> windows_core::Result<Option<CatalogPackage>> {
    let connection = reference.ConnectAsync()?;
    wait(&connection, Duration::from_secs(25))?;
    let result = connection.GetResults()?;
    if result.Status()? != ConnectResultStatus::Ok { return Err(windows_core::Error::from_hresult(result.ExtendedErrorCode()?)); }
    let options: FindPackagesOptions = create(0x572ded96_9c60_4526_8f92_ee7d91d38c1a)?;
    let filter: PackageMatchFilter = create(0xd02c9daf_99dc_429c_b503_4e504e4ab000)?;
    filter.SetField(PackageMatchField::Id)?;
    filter.SetOption(PackageFieldMatchOption::EqualsCaseInsensitive)?;
    filter.SetValue(&HSTRING::from(id))?;
    options.Filters()?.Append(&filter)?;
    let search = result.PackageCatalog()?.FindPackagesAsync(&options)?;
    wait(&search, Duration::from_secs(25))?;
    let matches = search.GetResults()?;
    if matches.Status()? != FindPackagesResultStatus::Ok { return Err(windows_core::Error::from_hresult(matches.ExtendedErrorCode()?)); }
    let matches = matches.Matches()?;
    if matches.Size()? != 1 { return Ok(None); }
    Ok(Some(matches.GetAt(0)?.CatalogPackage()?))
}

fn wait<T: windows_core::RuntimeType + 'static>(operation: &windows_future::IAsyncOperation<T>, timeout: Duration) -> windows_core::Result<()> {
    let deadline = Instant::now() + timeout;
    while operation.Status()? == windows_future::AsyncStatus::Started {
        if Instant::now() >= deadline {
            operation.Cancel()?;
            return Err(windows_core::Error::new(windows_core::HRESULT(0x800705B4u32 as i32), "WinGet operation timed out"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

pub fn attribute(item: &mut Installation) -> Result<bool, String> {
    let conventional = identity::normalize(&item.executable).contains("/winget/packages/");
    let _com = Apartment::new()?;
    let result = (|| -> windows_core::Result<bool> {
        let id = match item.app { AppKind::Codex => "OpenAI.Codex", AppKind::Claude => "Anthropic.ClaudeCode" };
        let reference = manager()?.GetLocalPackageCatalog(LocalPackageCatalog::InstalledPackages)?;
        let Some(package) = find(&reference, id)? else { return Ok(false); };
        let installed = package.InstalledVersion()?;
        let root = PathBuf::from(installed.GetMetadata(PackageVersionMetadataField::InstalledLocation)?.to_string());
        if root.as_os_str().is_empty() || !item.executable.starts_with(std::fs::canonicalize(&root).unwrap_or(root.clone())) { return Ok(false); }
        let scope = installed.GetMetadata(PackageVersionMetadataField::InstalledScope)?.to_string().to_lowercase();
        item.manager = Manager::Winget;
        item.root = root;
        let owned = item.root.file_name().is_some_and(|name| name.to_string_lossy()
            .eq_ignore_ascii_case(&format!("{id}_Microsoft.Winget.Source_8wekyb3d8bbwe")));
        if scope == "user" && owned { item.owner = Some(Owner::Winget { package: id.into(), scope }); }
        else { item.detail = Some("Confirm this installation's original WinGet source and scope with its administrator".into()); }
        Ok(true)
    })();
    match result {
        Ok(value) => Ok(value),
        Err(error) if conventional => Err(format!("WinGet identity: {error}")),
        Err(_) => Ok(false),
    }
}

fn remote(id: &str) -> windows_core::Result<(PackageManager, CatalogPackage)> {
    let manager = manager()?;
    let source = manager.GetPredefinedPackageCatalog(PredefinedPackageCatalog::OpenWindowsCatalog)?;
    let options: CreateCompositePackageCatalogOptions = create(0x526534b8_7e46_47c8_8416_b1685c327d37)?;
    options.Catalogs()?.Append(&source)?;
    options.SetCompositeSearchBehavior(CompositeSearchBehavior::RemotePackagesFromAllCatalogs)?;
    let reference = manager.CreateCompositePackageCatalog(&options)?;
    let package = find(&reference, id)?.ok_or_else(|| windows_core::Error::new(windows_core::HRESULT(0x80070490u32 as i32), "Exact WinGet package unavailable"))?;
    Ok((manager, package))
}

pub fn latest(id: &str) -> Result<String, String> {
    let _com = Apartment::new()?;
    remote(id).and_then(|(_, package)| package.DefaultInstallVersion()?.Version())
        .map(|v| v.to_string()).map_err(|e| format!("WinGet catalog: {e}"))
}

pub fn installing(app: AppKind) -> Result<bool, String> {
    let _com = Apartment::new()?;
    let id = match app { AppKind::Codex => "OpenAI.Codex", AppKind::Claude => "Anthropic.ClaudeCode" };
    manager().and_then(|manager| manager.GetLocalPackageCatalog(LocalPackageCatalog::InstallingPackages))
        .and_then(|reference| find(&reference, id)).map(|package| package.is_some()).map_err(|e| e.to_string())
}

pub fn update(id: &str, target: &str, scope: &str, root: &std::path::Path) -> Result<(), String> {
    let _com = Apartment::new()?;
    let result = (|| -> windows_core::Result<()> {
        let (manager, package) = remote(id)?;
        if scope != "user" || package.InstalledVersion()?.GetMetadata(PackageVersionMetadataField::InstalledScope)?.to_string().to_lowercase() != scope {
            return Err(windows_core::Error::new(windows_core::HRESULT(0x80070005u32 as i32), "Installation scope changed"));
        }
        let installed_root = PathBuf::from(package.InstalledVersion()?.GetMetadata(PackageVersionMetadataField::InstalledLocation)?.to_string());
        if identity::canonical_root(&installed_root) != identity::canonical_root(root) {
            return Err(windows_core::Error::new(windows_core::HRESULT(0x80070005u32 as i32), "WinGet installation location changed"));
        }
        let options: InstallOptions = create(0x1095f097_eb96_453b_b4e6_1613637f3b14)?;
        let versions = package.AvailableVersions()?;
        let mut selected = None;
        for index in 0..versions.Size()? {
            let candidate = versions.GetAt(index)?;
            if candidate.Version()?.to_string() == target { selected = Some(candidate); break; }
        }
        let selected = selected.ok_or_else(|| windows_core::Error::new(windows_core::HRESULT(0x80070490u32 as i32), "Planned version is no longer available"))?;
        options.SetPackageVersionId(&selected)?;
        options.SetPackageInstallScope(PackageInstallScope::User)?;
        options.SetPackageInstallMode(PackageInstallMode::Silent)?;
        let operation = manager.UpgradePackageAsync(&package, &options)?;
        // No forced cancellation after handing control to a package installer.
        while operation.Status()? == windows_future::AsyncStatus::Started { std::thread::sleep(Duration::from_millis(250)); }
        let result = operation.GetResults()?;
        if result.Status()? != InstallResultStatus::Ok { return Err(windows_core::Error::from_hresult(result.ExtendedErrorCode()?)); }
        if result.RebootRequired()? { return Err(windows_core::Error::new(windows_core::HRESULT(0x80070BC2u32 as i32), "A Windows restart is required; verify again afterwards")); }
        Ok(())
    })();
    result.map_err(|e| format!("WinGet update: {e}"))
}
