import path from 'node:path'

export interface NativeHostPathOptions {
  appPath: string
  isPackaged: boolean
  resourcesPath: string
  overridePath?: string
}

export function macOSHostCandidates(options: NativeHostPathOptions): readonly string[] {
  if (options.overridePath) {
    return [path.resolve(options.overridePath)]
  }

  if (options.isPackaged) {
    return [path.join(options.resourcesPath, 'macos-host', 'LumiereMacHost')]
  }

  const hostRoot = path.resolve(options.appPath, '../../hosts/macos/.build')
  return [
    path.join(hostRoot, 'debug', 'LumiereMacHost'),
    path.join(hostRoot, 'release', 'LumiereMacHost'),
  ]
}

export function windowsHostCandidates(options: NativeHostPathOptions): readonly string[] {
  if (options.overridePath) {
    return [path.win32.resolve(options.overridePath)]
  }

  if (options.isPackaged) {
    return [path.win32.join(options.resourcesPath, 'windows-host', 'Lumiere.Windows.Host.exe')]
  }

  const hostRoot = path.win32.resolve(
    options.appPath,
    '../../hosts/windows/src/Lumiere.Windows.Host/bin/x64',
  )
  const target = 'net10.0-windows10.0.26100.0/win-x64/Lumiere.Windows.Host.exe'
  return [path.win32.join(hostRoot, 'Debug', target), path.win32.join(hostRoot, 'Release', target)]
}
