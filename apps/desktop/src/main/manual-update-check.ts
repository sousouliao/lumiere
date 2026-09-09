import type { UpdateCheckResult } from '../shared/update-command'

interface LatestReleaseResponse {
  readonly ok: boolean
  readonly url?: string
  json(): Promise<unknown>
}

export type LatestReleaseRequest = () => Promise<LatestReleaseResponse>

interface ParsedVersion {
  core: readonly [number, number, number]
  prerelease: readonly string[]
}

const allowedReleaseOwners = new Set(['mournerliao', 'sousouliao'])

export async function checkLatestRelease(
  currentVersion: string,
  requestLatestRelease: LatestReleaseRequest,
  requestLatestReleasePage?: LatestReleaseRequest,
): Promise<UpdateCheckResult> {
  try {
    const response = await requestLatestRelease()
    if (response.ok) {
      const payload: unknown = await response.json()
      if (isObject(payload) && typeof payload.tag_name === 'string') {
        return compareLatestVersion(currentVersion, payload.tag_name)
      }
    }
  } catch {
    // Fall through to GitHub's non-REST latest-release redirect when the API is unavailable.
  }

  if (requestLatestReleasePage) {
    try {
      const response = await requestLatestReleasePage()
      const tag = response.ok && response.url ? releaseTagFromUrl(response.url) : null
      if (tag) return compareLatestVersion(currentVersion, tag)
    } catch {
      // The stable failed result intentionally hides network and GitHub implementation details.
    }
  }
  return { status: 'failed', currentVersion }
}

function compareLatestVersion(currentVersion: string, latestTag: string): UpdateCheckResult {
  const availableVersion = latestTag.replace(/^v/, '')
  if (compareVersions(availableVersion, currentVersion) > 0) {
    return { status: 'available', currentVersion, availableVersion }
  }
  return { status: 'up-to-date', currentVersion }
}

function releaseTagFromUrl(value: string): string | null {
  const url = new URL(value)
  const segments = url.pathname.split('/').filter(Boolean)
  if (
    url.protocol !== 'https:' ||
    url.hostname !== 'github.com' ||
    segments.length !== 5 ||
    !allowedReleaseOwners.has(segments[0].toLowerCase()) ||
    segments[1].toLowerCase() !== 'lumiere' ||
    segments[2] !== 'releases' ||
    segments[3] !== 'tag'
  ) {
    return null
  }
  return decodeURIComponent(segments[4])
}

export function compareVersions(left: string, right: string): number {
  const leftVersion = parseVersion(left)
  const rightVersion = parseVersion(right)
  if (!leftVersion || !rightVersion) throw new Error('Invalid semantic version.')

  for (let index = 0; index < leftVersion.core.length; index += 1) {
    const difference = leftVersion.core[index] - rightVersion.core[index]
    if (difference !== 0) return Math.sign(difference)
  }

  const leftPrerelease = leftVersion.prerelease
  const rightPrerelease = rightVersion.prerelease
  if (leftPrerelease.length === 0 || rightPrerelease.length === 0) {
    return leftPrerelease.length === rightPrerelease.length
      ? 0
      : leftPrerelease.length === 0
        ? 1
        : -1
  }

  const length = Math.min(leftPrerelease.length, rightPrerelease.length)
  for (let index = 0; index < length; index += 1) {
    const leftIdentifier = leftPrerelease[index]
    const rightIdentifier = rightPrerelease[index]
    if (leftIdentifier === rightIdentifier) continue
    const leftNumeric = /^\d+$/.test(leftIdentifier)
    const rightNumeric = /^\d+$/.test(rightIdentifier)
    if (leftNumeric && rightNumeric)
      return Math.sign(Number(leftIdentifier) - Number(rightIdentifier))
    if (leftNumeric !== rightNumeric) return leftNumeric ? -1 : 1
    return leftIdentifier < rightIdentifier ? -1 : 1
  }
  return Math.sign(leftPrerelease.length - rightPrerelease.length)
}

function parseVersion(version: string): ParsedVersion | null {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(version)
  if (!match) return null
  return {
    core: [Number(match[1]), Number(match[2]), Number(match[3])],
    prerelease: version.includes('-')
      ? version
          .slice(version.indexOf('-') + 1)
          .split('+', 1)[0]
          .split('.')
      : [],
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}
