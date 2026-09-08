import { EventEmitter } from 'node:events'
import { execPath } from 'node:process'
import { PassThrough } from 'node:stream'
import type { ChildProcessWithoutNullStreams } from 'node:child_process'
import { describe, expect, it } from 'vitest'
import { WindowsPlatformHost } from './windows-platform-host'

describe('Windows platform host process transport', () => {
  it('projects the current target and reuses one process for repeated folder capture', async () => {
    const process = new FakeNativeProcess()
    let spawnCount = 0
    let captureCount = 0
    process.stdin.on('data', (chunk: Buffer) => {
      const request = JSON.parse(chunk.toString('utf8')) as Record<string, unknown>
      const result =
        request.method === 'getCapabilities'
          ? {
              contractVersion: 4,
              platform: 'windows',
              hostStatus: 'available',
              captureModes: ['display'],
              deliveryTargets: ['folder'],
              hdrCapture: 'supported',
              outputProfiles: ['srgb-visual-match'],
            }
          : {
              status: 'completed',
              sourceDynamicRange: 'hdr',
              outputProfile: 'srgb-visual-match',
              deliveries: [
                {
                  target: 'folder',
                  status: 'success',
                  filePath: `C:\\Pictures\\Lumiere\\capture-${String(++captureCount)}.png`,
                },
              ],
            }
      process.respond({ version: 4, id: request.id, result })
    })

    const host = new WindowsPlatformHost([execPath], () => {
      spawnCount += 1
      return process.asChildProcess()
    })

    await expect(host.getCapabilities()).resolves.toMatchObject({
      platform: 'windows',
      hostStatus: 'available',
      captureModes: ['display'],
      deliveryTargets: ['folder'],
      hdrCapture: 'supported',
    })
    const firstCapture = await host.captureDisplay({ delivery: 'folder' })
    const secondCapture = await host.captureDisplay({ delivery: 'folder' })

    expect(firstCapture).toMatchObject({
      status: 'completed',
      sourceDynamicRange: 'hdr',
      deliveries: [{ filePath: 'C:\\Pictures\\Lumiere\\capture-1.png' }],
    })
    expect(secondCapture).toMatchObject({
      status: 'completed',
      sourceDynamicRange: 'hdr',
      deliveries: [{ filePath: 'C:\\Pictures\\Lumiere\\capture-2.png' }],
    })
    expect(spawnCount).toBe(1)

    host.dispose()
    expect(process.killed).toBe(true)
  })

  it('rejects a pending capture when disposal terminates the process', async () => {
    const process = new FakeNativeProcess()
    const requestWasWritten = new Promise<void>((resolve) => {
      process.stdin.once('data', () => {
        resolve()
      })
    })
    const host = new WindowsPlatformHost([execPath], () => process.asChildProcess())

    const capture = host.captureDisplay({ delivery: 'folder' })
    await requestWasWritten
    host.dispose()

    await expect(capture).resolves.toMatchObject({
      status: 'failed',
      failure: { code: 'host-unavailable', retryable: true },
    })
    expect(process.killed).toBe(true)
  })
})

class FakeNativeProcess extends EventEmitter {
  public readonly stdin = new PassThrough()
  public readonly stdout = new PassThrough()
  public readonly stderr = new PassThrough()
  public killed = false

  public respond(envelope: unknown): void {
    this.stdout.write(`${JSON.stringify(envelope)}\n`)
  }

  public kill(): boolean {
    this.killed = true
    return true
  }

  public asChildProcess(): ChildProcessWithoutNullStreams {
    return this as unknown as ChildProcessWithoutNullStreams
  }
}
