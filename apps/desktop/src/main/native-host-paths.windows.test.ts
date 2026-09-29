import { describe, expect, it } from 'vitest'
import { windowsHostCandidates } from './native-host-paths'

describe('Windows native host paths', () => {
  it('uses the packaged resources directory in production', () => {
    expect(
      windowsHostCandidates({
        appPath: String.raw`C:\Program Files\Lumiere\resources\app.asar`,
        isPackaged: true,
        resourcesPath: String.raw`C:\Program Files\Lumiere\resources`,
      }),
    ).toEqual([
      String.raw`C:\Program Files\Lumiere\resources\windows-host\Lumiere.Windows.Host.exe`,
    ])
  })

  it('prefers the current Debug build and falls back to Release during development', () => {
    expect(
      windowsHostCandidates({
        appPath: String.raw`D:\workspace\lumiere\apps\desktop`,
        isPackaged: false,
        resourcesPath: String.raw`D:\unused`,
      }),
    ).toEqual([
      String.raw`D:\workspace\lumiere\hosts\windows\src\Lumiere.Windows.Host\bin\x64\Debug\net10.0-windows10.0.26100.0\win-x64\Lumiere.Windows.Host.exe`,
      String.raw`D:\workspace\lumiere\hosts\windows\src\Lumiere.Windows.Host\bin\x64\Release\net10.0-windows10.0.26100.0\win-x64\Lumiere.Windows.Host.exe`,
    ])
  })

  it('honors an explicit development override', () => {
    expect(
      windowsHostCandidates({
        appPath: String.raw`D:\workspace\lumiere\apps\desktop`,
        isPackaged: false,
        resourcesPath: String.raw`D:\unused`,
        overridePath: String.raw`D:\hosts\Lumiere.Windows.Host.exe`,
      }),
    ).toEqual([String.raw`D:\hosts\Lumiere.Windows.Host.exe`])
  })
})
