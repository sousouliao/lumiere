import { randomUUID } from 'node:crypto'
import { access } from 'node:fs/promises'
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import type {
  CaptureResult,
  CommitRegionRequest,
  DeliveryResult,
  DisplayCaptureRequest,
  HostMethod,
  PlatformCapabilities,
  PlatformFailure,
  PlatformHost,
  PrepareRegionResult,
  ReleasedRegionCapture,
  LumierePlatform,
} from '../shared/platform-contract'
import { deliveryTargetsFor, PLATFORM_CONTRACT_VERSION } from '../shared/platform-contract'

const requestTimeoutMilliseconds = 15_000
const prepareRegionTimeoutMilliseconds = 30_000

export type SpawnHost = (executablePath: string) => ChildProcessWithoutNullStreams

interface PendingRequest {
  method: HostMethod
  resolve(value: unknown): void
  reject(error: Error): void
  timeout: NodeJS.Timeout
}

export class NativeProcessPlatformHost implements PlatformHost {
  private process: ChildProcessWithoutNullStreams | null = null
  private processStart: Promise<ChildProcessWithoutNullStreams> | null = null
  private stdoutBuffer = ''
  private readonly pending = new Map<string, PendingRequest>()
  private disposed = false

  public constructor(
    private readonly platform: LumierePlatform,
    private readonly executableCandidates: readonly string[],
    private readonly spawnHost: SpawnHost = spawnNativeHost,
  ) {}

  public async getCapabilities(): Promise<PlatformCapabilities> {
    try {
      return parseCapabilities(await this.request('getCapabilities', {}), this.platform)
    } catch (error) {
      return unavailableCapabilities(error, this.platform)
    }
  }

  public async captureDisplay(request: DisplayCaptureRequest): Promise<CaptureResult> {
    try {
      const result = parseCaptureResult(
        await this.request('captureDisplay', request),
        this.platform,
      )
      validateCaptureDeliveries(request, result)
      return result
    } catch (error) {
      return {
        status: 'failed',
        failure: failureFromError(error, this.platform),
      }
    }
  }

  public async prepareRegion(targetId: string): Promise<PrepareRegionResult> {
    try {
      return parsePrepareRegionResult(
        await this.request('prepareRegion', { targetId }),
        this.platform,
      )
    } catch (error) {
      return { status: 'failed', failure: failureFromError(error, this.platform) }
    }
  }

  public async commitRegion(request: CommitRegionRequest): Promise<CaptureResult> {
    try {
      const result = parseCaptureResult(await this.request('commitRegion', request), this.platform)
      validateCaptureDeliveries(request, result)
      return result
    } catch (error) {
      return { status: 'failed', failure: failureFromError(error, this.platform) }
    }
  }

  public async cancelRegion(sessionId: string): Promise<ReleasedRegionCapture> {
    try {
      return parseReleasedRegion(await this.request('cancelRegion', { sessionId }), this.platform)
    } catch {
      return { status: 'released' }
    }
  }

  public dispose(): void {
    this.disposed = true
    const child = this.process
    this.process = null
    if (child && !child.killed) {
      child.kill()
    }
    this.rejectPending(new Error(`The ${this.platform} native capture host was disposed.`))
  }

  private async request(method: HostMethod, params: object): Promise<unknown> {
    const child = await this.ensureProcess()
    const id = randomUUID()
    const line = JSON.stringify({
      version: PLATFORM_CONTRACT_VERSION,
      id,
      method,
      params,
    })

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(
        () => {
          this.handleTermination(
            child,
            new Error(`The ${this.platform} host timed out while handling ${method}.`),
          )
        },
        method === 'prepareRegion' ? prepareRegionTimeoutMilliseconds : requestTimeoutMilliseconds,
      )
      this.pending.set(id, { method, resolve, reject, timeout })

      child.stdin.write(`${line}\n`, (error) => {
        if (!error) {
          return
        }
        clearTimeout(timeout)
        this.pending.delete(id)
        reject(error)
      })
    })
  }

  private async ensureProcess(): Promise<ChildProcessWithoutNullStreams> {
    if (this.disposed) {
      throw new Error(`The ${this.platform} native capture host was disposed.`)
    }
    if (this.process) {
      return this.process
    }
    if (this.processStart) {
      return this.processStart
    }

    const processStart = this.startProcess()
    this.processStart = processStart
    try {
      return await processStart
    } finally {
      if (this.processStart === processStart) {
        this.processStart = null
      }
    }
  }

  private async startProcess(): Promise<ChildProcessWithoutNullStreams> {
    const executablePath = await firstAccessiblePath(this.executableCandidates)
    if (!executablePath) {
      throw new Error(`The ${this.platform} native capture host executable is unavailable.`)
    }
    if (this.disposed) {
      throw new Error(`The ${this.platform} native capture host was disposed.`)
    }

    const child = this.spawnHost(executablePath)
    this.process = child

    child.stdout.setEncoding('utf8')
    child.stdout.on('data', (chunk: string) => {
      this.acceptStdout(child, chunk)
    })
    child.stderr.setEncoding('utf8')
    child.stderr.on('data', (chunk: string) => {
      for (const line of chunk.split('\n').filter(Boolean)) {
        process.stderr.write(`[${this.platform}-host] ${line}\n`)
      }
    })
    child.on('error', (error) => {
      this.handleTermination(child, error)
    })
    child.on('exit', (code, signal) => {
      this.handleTermination(
        child,
        new Error(
          `The ${this.platform} native capture host exited (code=${String(code)}, signal=${String(signal)}).`,
        ),
      )
    })

    return child
  }

  private acceptStdout(child: ChildProcessWithoutNullStreams, chunk: string): void {
    if (this.process !== child) {
      return
    }
    this.stdoutBuffer += chunk
    let newlineIndex = this.stdoutBuffer.indexOf('\n')
    while (newlineIndex >= 0) {
      const line = this.stdoutBuffer.slice(0, newlineIndex)
      this.stdoutBuffer = this.stdoutBuffer.slice(newlineIndex + 1)
      if (line.length > 0) {
        this.acceptResponseLine(child, line)
      }
      newlineIndex = this.stdoutBuffer.indexOf('\n')
    }
  }

  private acceptResponseLine(child: ChildProcessWithoutNullStreams, line: string): void {
    let envelope: unknown
    try {
      envelope = JSON.parse(line)
    } catch {
      this.handleTermination(child, new Error(`The ${this.platform} host emitted invalid JSON.`))
      return
    }

    if (
      !isRecord(envelope) ||
      envelope.version !== PLATFORM_CONTRACT_VERSION ||
      typeof envelope.id !== 'string' ||
      envelope.id.length === 0 ||
      (!hasExactKeys(envelope, ['version', 'id', 'result']) &&
        !hasExactKeys(envelope, ['version', 'id', 'error']))
    ) {
      this.handleTermination(
        child,
        new Error(`The ${this.platform} host emitted an invalid protocol envelope.`),
      )
      return
    }

    const pending = this.pending.get(envelope.id)
    if (!pending) {
      return
    }

    try {
      const value =
        'error' in envelope
          ? new NativeHostFailure(parseFailure(envelope.error))
          : parseHostResult(pending.method, envelope.result, this.platform)
      clearTimeout(pending.timeout)
      this.pending.delete(envelope.id)
      if (value instanceof NativeHostFailure) {
        pending.reject(value)
      } else {
        pending.resolve(value)
      }
    } catch (error) {
      this.handleTermination(child, error instanceof Error ? error : new Error(String(error)))
    }
  }

  private handleTermination(child: ChildProcessWithoutNullStreams, error: Error): void {
    if (this.process !== child) {
      return
    }
    this.process = null
    this.stdoutBuffer = ''
    if (!child.killed) {
      child.kill()
    }
    this.rejectPending(error)
  }

  private rejectPending(error: Error): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout)
      pending.reject(error)
    }
    this.pending.clear()
  }
}

function spawnNativeHost(executablePath: string): ChildProcessWithoutNullStreams {
  return spawn(executablePath, [], {
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true,
  })
}

class NativeHostFailure extends Error {
  public constructor(public readonly failure: PlatformFailure) {
    super(failure.message)
    this.name = 'NativeHostFailure'
  }
}

async function firstAccessiblePath(candidates: readonly string[]): Promise<string | null> {
  for (const candidate of candidates) {
    try {
      await access(candidate)
      return candidate
    } catch {
      // Continue to the next explicit candidate.
    }
  }
  return null
}

function unavailableCapabilities(error: unknown, platform: LumierePlatform): PlatformCapabilities {
  return {
    contractVersion: PLATFORM_CONTRACT_VERSION,
    platform,
    hostStatus: 'unavailable',
    captureModes: [],
    deliveryTargets: [],
    hdrCapture: 'unavailable',
    outputProfiles: ['srgb-visual-match'],
    unavailableReason: failureFromError(error, platform),
  }
}

function failureFromError(error: unknown, platform: LumierePlatform): PlatformFailure {
  if (error instanceof NativeHostFailure) {
    return error.failure
  }
  return {
    code: 'host-unavailable',
    message:
      error instanceof Error
        ? error.message
        : `The ${platform} native capture host is unavailable.`,
    retryable: true,
  }
}

function parseCapabilities(value: unknown, platform: LumierePlatform): PlatformCapabilities {
  if (!isRecord(value)) {
    throw new Error(`The ${platform} host returned invalid capabilities.`)
  }
  const captureModes = value.captureModes
  const deliveryTargets = value.deliveryTargets
  const outputProfiles = value.outputProfiles
  const activeTarget = value.activeTarget
  if (
    value.contractVersion !== PLATFORM_CONTRACT_VERSION ||
    value.platform !== platform ||
    (value.hostStatus !== 'available' && value.hostStatus !== 'unavailable') ||
    !Array.isArray(captureModes) ||
    !captureModes.every((mode) => mode === 'region' || mode === 'display') ||
    new Set(captureModes).size !== captureModes.length ||
    !Array.isArray(deliveryTargets) ||
    !deliveryTargets.every((target) => target === 'clipboard' || target === 'folder') ||
    new Set(deliveryTargets).size !== deliveryTargets.length ||
    (value.hdrCapture !== 'supported' &&
      value.hdrCapture !== 'unavailable' &&
      value.hdrCapture !== 'unvalidated') ||
    !Array.isArray(outputProfiles) ||
    outputProfiles.length !== 1 ||
    outputProfiles[0] !== 'srgb-visual-match' ||
    !hasExactKeys(
      value,
      [
        'contractVersion',
        'platform',
        'hostStatus',
        'captureModes',
        'deliveryTargets',
        'hdrCapture',
        'outputProfiles',
      ],
      ['activeTarget', 'unavailableReason'],
    ) ||
    (value.hostStatus === 'unavailable' && value.unavailableReason === undefined)
  ) {
    throw new Error(`The ${platform} host returned invalid capabilities.`)
  }

  if (value.unavailableReason !== undefined) {
    parseFailure(value.unavailableReason)
  }
  if (
    activeTarget !== undefined &&
    (!isRecord(activeTarget) ||
      typeof activeTarget.id !== 'string' ||
      activeTarget.id.length === 0 ||
      !isLogicalSize(activeTarget.logicalSize) ||
      !hasExactKeys(activeTarget, ['id', 'logicalSize']))
  ) {
    throw new Error(`The ${platform} host returned an invalid active capture target.`)
  }
  return value as unknown as PlatformCapabilities
}

function parseHostResult(
  method: HostMethod,
  value: unknown,
  platform: LumierePlatform,
): PlatformCapabilities | CaptureResult | PrepareRegionResult | ReleasedRegionCapture {
  switch (method) {
    case 'getCapabilities':
      return parseCapabilities(value, platform)
    case 'captureDisplay':
    case 'commitRegion':
      return parseCaptureResult(value, platform)
    case 'prepareRegion':
      return parsePrepareRegionResult(value, platform)
    case 'cancelRegion':
      return parseReleasedRegion(value, platform)
  }
}

function parsePrepareRegionResult(value: unknown, platform: LumierePlatform): PrepareRegionResult {
  if (isRecord(value) && value.status === 'failed') {
    return parseCaptureResult(value, platform) as PrepareRegionResult
  }
  if (
    !isRecord(value) ||
    value.status !== 'prepared' ||
    typeof value.sessionId !== 'string' ||
    value.sessionId.length === 0 ||
    !isLogicalSize(value.targetLogicalSize) ||
    !isRecord(value.preview) ||
    typeof value.preview.filePath !== 'string' ||
    value.preview.filePath.length === 0 ||
    value.preview.mediaType !== 'image/png' ||
    !isPixelSize(value.preview.pixelSize) ||
    !Number.isSafeInteger(value.leaseMilliseconds) ||
    (value.leaseMilliseconds as number) <= 0 ||
    !hasExactKeys(value.preview, ['filePath', 'mediaType', 'pixelSize']) ||
    !hasExactKeys(value, [
      'status',
      'sessionId',
      'targetLogicalSize',
      'preview',
      'leaseMilliseconds',
    ])
  ) {
    throw new Error(`The ${platform} host returned an invalid prepared Region capture.`)
  }
  return value as unknown as PrepareRegionResult
}

function parseReleasedRegion(value: unknown, platform: LumierePlatform): ReleasedRegionCapture {
  if (!isRecord(value) || value.status !== 'released' || !hasExactKeys(value, ['status'])) {
    throw new Error(`The ${platform} host returned an invalid Region release result.`)
  }
  return { status: 'released' }
}

function parseCaptureResult(value: unknown, platform: LumierePlatform): CaptureResult {
  if (!isRecord(value) || typeof value.status !== 'string') {
    throw new Error(`The ${platform} host returned an invalid capture result.`)
  }
  if (value.status === 'failed') {
    if (!hasExactKeys(value, ['status', 'failure'])) {
      throw new Error(`The ${platform} host returned an invalid capture result.`)
    }
    return { status: 'failed', failure: parseFailure(value.failure) }
  }
  if (value.status === 'cancelled') {
    if (!hasExactKeys(value, ['status'])) {
      throw new Error(`The ${platform} host returned an invalid capture result.`)
    }
    return { status: 'cancelled' }
  }
  if (
    value.status !== 'completed' ||
    (value.sourceDynamicRange !== 'sdr' && value.sourceDynamicRange !== 'hdr') ||
    value.outputProfile !== 'srgb-visual-match' ||
    !Array.isArray(value.deliveries) ||
    value.deliveries.length === 0 ||
    !hasExactKeys(value, ['status', 'sourceDynamicRange', 'outputProfile', 'deliveries'])
  ) {
    throw new Error(`The ${platform} host returned an invalid capture result.`)
  }
  const deliveries = value.deliveries.map(parseDeliveryResult)
  if (new Set(deliveries.map(({ target }) => target)).size !== deliveries.length) {
    throw new Error(`The ${platform} host returned duplicate delivery results.`)
  }
  return {
    status: 'completed',
    sourceDynamicRange: value.sourceDynamicRange,
    outputProfile: 'srgb-visual-match',
    deliveries,
  }
}

function parseDeliveryResult(value: unknown): DeliveryResult {
  if (!isRecord(value) || (value.target !== 'clipboard' && value.target !== 'folder')) {
    throw new Error('The native host returned an invalid delivery result.')
  }
  if (value.status === 'failed' && hasExactKeys(value, ['target', 'status', 'failure'])) {
    return { target: value.target, status: 'failed', failure: parseFailure(value.failure) }
  }
  if (value.status !== 'success') {
    throw new Error('The native host returned an invalid delivery result.')
  }
  if (value.target === 'clipboard' && hasExactKeys(value, ['target', 'status'])) {
    return { target: 'clipboard', status: 'success' }
  }
  if (
    value.target === 'folder' &&
    typeof value.filePath === 'string' &&
    value.filePath.length > 0 &&
    hasExactKeys(value, ['target', 'status', 'filePath'])
  ) {
    return { target: 'folder', status: 'success', filePath: value.filePath }
  }
  throw new Error('The native host returned an invalid delivery result.')
}

function validateCaptureDeliveries(
  request: { delivery: DisplayCaptureRequest['delivery'] },
  result: CaptureResult,
): void {
  if (result.status !== 'completed') {
    return
  }
  const expected = deliveryTargetsFor(request.delivery)
  if (
    result.deliveries.length !== expected.length ||
    !expected.every((target) => result.deliveries.some((delivery) => delivery.target === target))
  ) {
    throw new Error('The native host returned delivery results that do not match the request.')
  }
}

function isLogicalSize(value: unknown): boolean {
  if (
    !isRecord(value) ||
    !isPositiveFiniteNumber(value.width) ||
    !isPositiveFiniteNumber(value.height) ||
    !hasExactKeys(value, ['width', 'height'])
  ) {
    return false
  }
  return true
}

function isPixelSize(value: unknown): boolean {
  return (
    isRecord(value) &&
    Number.isSafeInteger(value.width) &&
    (value.width as number) > 0 &&
    Number.isSafeInteger(value.height) &&
    (value.height as number) > 0 &&
    hasExactKeys(value, ['width', 'height'])
  )
}

function parseFailure(value: unknown): PlatformFailure {
  if (!isRecord(value)) {
    throw new Error('The native host returned an invalid failure.')
  }
  const validCodes = [
    'host-unavailable',
    'permission-denied',
    'capture-unavailable',
    'delivery-unavailable',
    'delivery-failed',
    'invalid-request',
    'unexpected-failure',
  ]
  if (
    typeof value.code !== 'string' ||
    !validCodes.includes(value.code) ||
    typeof value.message !== 'string' ||
    value.message.length === 0 ||
    typeof value.retryable !== 'boolean' ||
    !hasExactKeys(value, ['code', 'message', 'retryable'])
  ) {
    throw new Error('The native host returned an invalid failure.')
  }
  return value as unknown as PlatformFailure
}

function isPositiveFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasExactKeys(
  value: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[] = [],
): boolean {
  const keys = Object.keys(value)
  const allowed = new Set([...required, ...optional])
  return required.every((key) => key in value) && keys.every((key) => allowed.has(key))
}
