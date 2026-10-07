import { EventEmitter } from 'node:events'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { WindowsUpdateService } from './windows-update-service'

class Client extends EventEmitter {
  checkForUpdates = vi.fn(() => Promise.resolve({ updateInfo: { version: '0.6.1' } }))
  downloadUpdate = vi.fn(() => Promise.resolve(['installer.exe']))
  quitAndInstall = vi.fn()
}

const services: WindowsUpdateService[] = []
function fixture(client: Client = new Client()) {
  const changed = vi.fn()
  const busy = vi.fn(() => false)
  const stop = vi.fn(() => Promise.resolve())
  const resume = vi.fn()
  const service = new WindowsUpdateService('0.6.0', client, changed, busy, stop, resume)
  services.push(service)
  return { client, service, changed, busy, stop, resume }
}
afterEach(() => {
  services.splice(0).forEach((service) => {
    service.dispose()
  })
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('Windows update lifetime', () => {
  it('checks periodically without downloading or installing and stops its clock on exit', async () => {
    vi.useFakeTimers()
    const { service, client } = fixture()
    await vi.advanceTimersByTimeAsync(29_999)
    expect(client.checkForUpdates).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(client.checkForUpdates).toHaveBeenCalledTimes(1)
    expect(service.getSnapshot().windowsUpdate).toEqual({
      status: 'available',
      availableVersion: '0.6.1',
    })
    await vi.advanceTimersByTimeAsync(6 * 60 * 60 * 1_000)
    expect(client.checkForUpdates).toHaveBeenCalledTimes(2)
    expect(client.downloadUpdate).not.toHaveBeenCalled()
    expect(client.quitAndInstall).not.toHaveBeenCalled()
    service.dispose()
    await vi.advanceTimersByTimeAsync(6 * 60 * 60 * 1_000)
    expect(client.checkForUpdates).toHaveBeenCalledTimes(2)
  })

  it('serializes requests, publishes progress and keeps the downloaded update ready', async () => {
    const { service, client, changed } = fixture()
    const checking = service.check()
    expect(service.check()).toBe(checking)
    await checking
    let finish!: (files: string[]) => void
    client.downloadUpdate.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve
        }),
    )
    const download = service.download()
    expect(service.download()).toBe(download)
    await Promise.resolve()
    client.emit('download-progress', { percent: 42.9 })
    expect(changed).toHaveBeenLastCalledWith({
      currentVersion: '0.6.0',
      windowsUpdate: { status: 'downloading', availableVersion: '0.6.1', percent: 42.9 },
    })
    finish(['installer.exe'])
    await download
    await service.check()
    expect(client.checkForUpdates).toHaveBeenCalledTimes(1)
    expect(service.getSnapshot().windowsUpdate?.status).toBe('ready')
  })

  it('rejects installing during capture, blocks new captures immediately and waits for Host exit', async () => {
    const { service, client, busy, stop } = fixture()
    await service.check()
    await service.download()
    busy.mockReturnValue(true)
    await service.install()
    expect(stop).not.toHaveBeenCalled()
    busy.mockReturnValue(false)
    let finish!: () => void
    stop.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve
        }),
    )
    const installing = service.install()
    expect(service.isInstalling()).toBe(true)
    expect(service.install()).toBe(installing)
    await Promise.resolve()
    expect(client.quitAndInstall).not.toHaveBeenCalled()
    finish()
    await installing
    expect(client.quitAndInstall).toHaveBeenCalledExactlyOnceWith(true, true)
  })

  it('offers the failed operation for retry without installing an unverified download', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const { service, client } = fixture()
    client.checkForUpdates.mockRejectedValueOnce(new Error('offline'))
    await service.check()
    expect(service.getSnapshot().windowsUpdate).toMatchObject({ status: 'failed', retry: 'check' })
    await service.check()
    client.downloadUpdate.mockRejectedValueOnce(new Error('checksum mismatch'))
    await service.download()
    expect(service.getSnapshot().windowsUpdate).toMatchObject({
      status: 'failed',
      retry: 'download',
    })
    await service.install()
    expect(client.quitAndInstall).not.toHaveBeenCalled()
    await service.download()
    expect(service.getSnapshot().windowsUpdate?.status).toBe('ready')
  })

  it('never checks or installs when the packaged update configuration is absent', async () => {
    const service = new WindowsUpdateService(
      '0.6.0',
      null,
      vi.fn(),
      () => false,
      () => Promise.resolve(),
      vi.fn(),
    )
    services.push(service)
    await service.check()
    await service.download()
    await service.install()
    expect(service.getSnapshot().windowsUpdate?.status).toBe('disabled')
  })

  it('leaves a failed Host shutdown retryable and resumes capture without running the installer', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const { service, client, stop, resume } = fixture()
    await service.check()
    await service.download()
    stop.mockRejectedValueOnce(new Error('Host did not exit'))
    await service.install()
    expect(client.quitAndInstall).not.toHaveBeenCalled()
    expect(resume).toHaveBeenCalledOnce()
    expect(service.isInstalling()).toBe(false)
    expect(service.getSnapshot().windowsUpdate).toMatchObject({
      status: 'failed',
      retry: 'install',
    })
    await service.install()
    expect(client.quitAndInstall).toHaveBeenCalledExactlyOnceWith(true, true)
  })

  it('handles an installer error event without leaving capture locked', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const { service, client, resume } = fixture()
    await service.check()
    await service.download()
    client.quitAndInstall.mockImplementation(() => {
      client.emit('error', new Error('Installer unavailable'))
    })
    await service.install()
    expect(service.isInstalling()).toBe(false)
    expect(resume).toHaveBeenCalledOnce()
    expect(service.getSnapshot().windowsUpdate).toMatchObject({
      status: 'failed',
      retry: 'install',
    })
  })
})
