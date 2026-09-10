import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { afterEach, describe, expect, it } from 'vitest'
import { SettingsStore } from './settings-store'

const temporaryDirectories: string[] = []

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((path) => rm(path, { recursive: true })))
})

describe('SettingsStore', () => {
  it('defaults to clipboard and folder when no settings file exists', async () => {
    const store = new SettingsStore(await settingsPath())

    await store.load()

    expect(store.getCapturePreferences()).toEqual({
      delivery: 'both',
      hdrStatusReminders: true,
    })
    expect(store.getCaptureShortcuts()).toEqual({ region: null, display: null })
    expect(store.getSaveDirectory()).toBeNull()
    expect(store.getAfterCaptureBehavior()).toBe('do-nothing')
    expect(store.getHdrStatusReminders()).toBe(true)
  })

  it('persists an output delivery and restores it in a new store', async () => {
    const filePath = await settingsPath()
    const store = new SettingsStore(filePath)
    await store.load()

    await store.setOutputDelivery('folder')
    const restartedStore = new SettingsStore(filePath)
    await restartedStore.load()

    expect(restartedStore.getOutputDelivery()).toBe('folder')
    await expect(readFile(filePath, 'utf8')).resolves.toContain('"outputDelivery": "folder"')
  })

  it('falls back to the default for an invalid or unsupported settings file', async () => {
    const filePath = await settingsPath()
    await writeFile(filePath, '{"version":3,"outputDelivery":"clipboard"}', 'utf8')
    const store = new SettingsStore(filePath)

    await store.load()

    expect(store.getOutputDelivery()).toBe('both')
  })

  it('migrates v1 output settings when a shortcut is first saved', async () => {
    const filePath = await settingsPath()
    await writeFile(filePath, '{"version":1,"outputDelivery":"folder"}', 'utf8')
    const store = new SettingsStore(filePath)
    await store.load()

    await store.setCaptureShortcut('region', 'Command+Shift+L')

    expect(store.getOutputDelivery()).toBe('folder')
    expect(store.getCaptureShortcuts()).toEqual({
      region: 'Command+Shift+L',
      display: null,
    })
    await expect(readFile(filePath, 'utf8')).resolves.toContain('"version": 5')
  })

  it('migrates v2 settings and persists an after-capture behavior', async () => {
    const filePath = await settingsPath()
    await writeFile(
      filePath,
      JSON.stringify({
        version: 2,
        outputDelivery: 'folder',
        captureShortcuts: { region: 'Command+Shift+L', display: null },
      }),
      'utf8',
    )
    const store = new SettingsStore(filePath)
    await store.load()

    expect(store.getAfterCaptureBehavior()).toBe('do-nothing')
    await store.setAfterCaptureBehavior('show-in-folder')

    const restartedStore = new SettingsStore(filePath)
    await restartedStore.load()
    expect(restartedStore.getOutputDelivery()).toBe('folder')
    expect(restartedStore.getCaptureShortcuts()).toEqual({
      region: 'Command+Shift+L',
      display: null,
    })
    expect(restartedStore.getAfterCaptureBehavior()).toBe('show-in-folder')
    await expect(readFile(filePath, 'utf8')).resolves.toContain('"version": 5')
  })

  it('migrates v3 settings and persists the HDR status reminder preference', async () => {
    const filePath = await settingsPath()
    await writeFile(
      filePath,
      JSON.stringify({
        version: 3,
        outputDelivery: 'both',
        captureShortcuts: { region: null, display: null },
        afterCaptureBehavior: 'show-in-folder',
      }),
      'utf8',
    )
    const store = new SettingsStore(filePath)
    await store.load()

    expect(store.getHdrStatusReminders()).toBe(true)
    await store.setHdrStatusReminders(false)

    const restartedStore = new SettingsStore(filePath)
    await restartedStore.load()
    expect(restartedStore.getAfterCaptureBehavior()).toBe('show-in-folder')
    expect(restartedStore.getHdrStatusReminders()).toBe(false)
    await expect(readFile(filePath, 'utf8')).resolves.toContain('"version": 5')
  })

  it('migrates v4 settings and persists a custom save directory', async () => {
    const filePath = await settingsPath()
    await writeFile(
      filePath,
      JSON.stringify({
        version: 5,
        outputDelivery: 'both',
        captureShortcuts: { region: null, display: null },
        afterCaptureBehavior: 'do-nothing',
        hdrStatusReminders: true,
      }),
      'utf8',
    )
    const store = new SettingsStore(filePath)
    await store.load()

    expect(store.getSaveDirectory()).toBeNull()
    await store.setSaveDirectory('/Users/example/Pictures/Screenshots')

    const restartedStore = new SettingsStore(filePath)
    await restartedStore.load()
    expect(restartedStore.getSaveDirectory()).toBe('/Users/example/Pictures/Screenshots')
    expect(restartedStore.getCapturePreferences()).toMatchObject({
      delivery: 'both',
      saveDirectory: '/Users/example/Pictures/Screenshots',
    })
    await expect(readFile(filePath, 'utf8')).resolves.toContain('"version": 5')
  })
})

async function settingsPath(): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), 'lumiere-settings-'))
  temporaryDirectories.push(directory)
  return join(directory, 'settings.json')
}
