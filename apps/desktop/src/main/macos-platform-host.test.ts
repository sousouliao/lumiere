import { EventEmitter } from 'node:events'
import { execPath } from 'node:process'
import { PassThrough } from 'node:stream'
import type { ChildProcessWithoutNullStreams } from 'node:child_process'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MacOSPlatformHost } from './macos-platform-host'

afterEach(() => {
  vi.useRealTimers()
})

describe('macOS platform host process transport', () => {
  it('correlates concurrent JSON Lines responses by request id', async () => {
    const process = new FakeNativeProcess()
    let spawnCount = 0
    const requests: Record<string, unknown>[] = []
    process.stdin.on('data', (chunk: Buffer) => {
      for (const line of chunk.toString('utf8').trim().split('\n')) {
        requests.push(JSON.parse(line) as Record<string, unknown>)
      }
      if (requests.length !== 2) {
        return
      }

      const [capabilitiesRequest, captureRequest] = requests
      process.respond({
        version: 4,
        id: captureRequest.id,
        result: {
          status: 'completed',
          sourceDynamicRange: 'hdr',
          outputProfile: 'srgb-visual-match',
          deliveries: [{ target: 'folder', status: 'success', filePath: '/tmp/capture.png' }],
        },
      })
      process.respond({
        version: 4,
        id: capabilitiesRequest.id,
        result: {
          contractVersion: 4,
          platform: 'macos',
          hostStatus: 'available',
          captureModes: ['display'],
          deliveryTargets: ['folder'],
          hdrCapture: 'supported',
          outputProfiles: ['srgb-visual-match'],
        },
      })
    })

    const host = new MacOSPlatformHost([execPath], () => {
      spawnCount += 1
      return process.asChildProcess()
    })
    const capabilitiesPromise = host.getCapabilities()
    const capturePromise = host.captureDisplay({ delivery: 'folder' })

    await expect(capabilitiesPromise).resolves.toMatchObject({
      hostStatus: 'available',
      captureModes: ['display'],
    })
    await expect(capturePromise).resolves.toMatchObject({
      status: 'completed',
      deliveries: [{ filePath: '/tmp/capture.png' }],
    })
    expect(spawnCount).toBe(1)
    host.dispose()
  })

  it('terminates a host that times out so the next request can restart cleanly', async () => {
    vi.useFakeTimers()
    const firstProcess = new FakeNativeProcess()
    const secondProcess = new FakeNativeProcess()
    const processes = [firstProcess, secondProcess]
    const host = new MacOSPlatformHost([execPath], () => {
      const process = processes.shift()
      if (!process) {
        throw new Error('Unexpected extra host spawn.')
      }
      return process.asChildProcess()
    })
    const requestWasWritten = new Promise<void>((resolve) => {
      firstProcess.stdin.once('data', () => {
        resolve()
      })
    })

    const capture = host.captureDisplay({ delivery: 'folder' })
    await requestWasWritten
    await vi.advanceTimersByTimeAsync(15_000)

    await expect(capture).resolves.toMatchObject({
      status: 'failed',
      failure: { code: 'host-unavailable', retryable: true },
    })
    expect(firstProcess.killed).toBe(true)

    secondProcess.stdin.once('data', (chunk: Buffer) => {
      const request = JSON.parse(chunk.toString('utf8')) as Record<string, unknown>
      secondProcess.respond({
        version: 4,
        id: request.id,
        result: {
          contractVersion: 4,
          platform: 'macos',
          hostStatus: 'available',
          captureModes: ['display'],
          deliveryTargets: ['folder'],
          hdrCapture: 'unavailable',
          outputProfiles: ['srgb-visual-match'],
        },
      })
    })
    await expect(host.getCapabilities()).resolves.toMatchObject({ hostStatus: 'available' })
    host.dispose()
  })

  it('rejects response envelopes containing both result and error', async () => {
    const process = new FakeNativeProcess()
    process.stdin.once('data', (chunk: Buffer) => {
      const request = JSON.parse(chunk.toString('utf8')) as Record<string, unknown>
      process.respond({
        version: 4,
        id: request.id,
        result: { status: 'cancelled' },
        error: {
          code: 'permission-denied',
          message: 'This result must not be accepted.',
          retryable: false,
        },
      })
    })
    const host = new MacOSPlatformHost([execPath], () => process.asChildProcess())

    await expect(host.captureDisplay({ delivery: 'folder' })).resolves.toMatchObject({
      status: 'failed',
      failure: { code: 'host-unavailable', retryable: true },
    })
    expect(process.killed).toBe(true)
  })

  it('rejects unknown fields inside a capture result', async () => {
    const process = new FakeNativeProcess()
    process.stdin.once('data', (chunk: Buffer) => {
      const request = JSON.parse(chunk.toString('utf8')) as Record<string, unknown>
      process.respond({
        version: 4,
        id: request.id,
        result: { status: 'cancelled', unexpected: true },
      })
    })
    const host = new MacOSPlatformHost([execPath], () => process.asChildProcess())

    await expect(host.captureDisplay({ delivery: 'folder' })).resolves.toMatchObject({
      status: 'failed',
      failure: { code: 'host-unavailable', retryable: true },
    })
    expect(process.killed).toBe(true)
  })

  it('does not let a stale child exit terminate its replacement', async () => {
    const firstProcess = new FakeNativeProcess()
    const secondProcess = new FakeNativeProcess()
    const processes = [firstProcess, secondProcess]
    const host = new MacOSPlatformHost([execPath], () => {
      const process = processes.shift()
      if (!process) {
        throw new Error('Unexpected extra host spawn.')
      }
      return process.asChildProcess()
    })

    firstProcess.stdin.once('data', () => {
      firstProcess.emit('exit', 17, null)
    })
    await expect(host.captureDisplay({ delivery: 'folder' })).resolves.toMatchObject({
      status: 'failed',
      failure: { code: 'host-unavailable' },
    })

    secondProcess.stdin.once('data', (chunk: Buffer) => {
      const request = JSON.parse(chunk.toString('utf8')) as Record<string, unknown>
      firstProcess.emit('exit', 17, null)
      secondProcess.respond({
        version: 4,
        id: request.id,
        result: {
          contractVersion: 4,
          platform: 'macos',
          hostStatus: 'available',
          captureModes: ['display'],
          deliveryTargets: ['folder'],
          hdrCapture: 'supported',
          outputProfiles: ['srgb-visual-match'],
        },
      })
    })

    await expect(host.getCapabilities()).resolves.toMatchObject({ hostStatus: 'available' })
    expect(secondProcess.killed).toBe(false)
    host.dispose()
  })

  it('converts an unexpected host exit into an unavailable capture result', async () => {
    const process = new FakeNativeProcess()
    process.stdin.once('data', () => {
      process.emit('exit', 17, null)
    })
    const host = new MacOSPlatformHost([execPath], () => process.asChildProcess())

    await expect(host.captureDisplay({ delivery: 'folder' })).resolves.toMatchObject({
      status: 'failed',
      failure: { code: 'host-unavailable', retryable: true },
    })
  })
})

class FakeNativeProcess extends EventEmitter {
  public readonly stdin = new PassThrough()
  public readonly stdout = new PassThrough()
  public readonly stderr = new PassThrough()
  public killed = false

  public kill(): boolean {
    this.killed = true
    return true
  }

  public respond(value: unknown): void {
    this.stdout.write(`${JSON.stringify(value)}\n`)
  }

  public asChildProcess(): ChildProcessWithoutNullStreams {
    return this as unknown as ChildProcessWithoutNullStreams
  }
}
