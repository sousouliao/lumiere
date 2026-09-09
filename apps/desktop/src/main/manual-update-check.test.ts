import { describe, expect, it } from 'vitest'
import { checkLatestRelease, compareVersions } from './manual-update-check'

describe('manual update check', () => {
  it('reports a newer stable GitHub release', async () => {
    await expect(
      checkLatestRelease('0.2.0', () =>
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ tag_name: 'v0.3.0' }),
        }),
      ),
    ).resolves.toEqual({
      status: 'available',
      currentVersion: '0.2.0',
      availableVersion: '0.3.0',
    })
  })

  it('treats the same or an older release as up to date', async () => {
    await expect(
      checkLatestRelease('0.3.0', () =>
        Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ tag_name: 'v0.3.0' }),
        }),
      ),
    ).resolves.toEqual({ status: 'up-to-date', currentVersion: '0.3.0' })
  })

  it('returns a stable failure for network and payload errors', async () => {
    await expect(
      checkLatestRelease('0.2.0', () => Promise.reject(new Error('offline'))),
    ).resolves.toEqual({ status: 'failed', currentVersion: '0.2.0' })
    await expect(
      checkLatestRelease('0.2.0', () =>
        Promise.resolve({ ok: true, json: () => Promise.resolve({}) }),
      ),
    ).resolves.toEqual({ status: 'failed', currentVersion: '0.2.0' })
  })

  it('falls back to the official latest-release redirect when the API is rate limited', async () => {
    await expect(
      checkLatestRelease(
        '0.2.0-preview.1',
        () => Promise.resolve({ ok: false, json: () => Promise.resolve({}) }),
        () =>
          Promise.resolve({
            ok: true,
            url: 'https://github.com/sousouliao/lumiere/releases/tag/v0.2.0',
            json: () => Promise.resolve({}),
          }),
      ),
    ).resolves.toEqual({
      status: 'available',
      currentVersion: '0.2.0-preview.1',
      availableVersion: '0.2.0',
    })
  })

  it('rejects latest-release redirects outside the known Lumiere repositories', async () => {
    await expect(
      checkLatestRelease(
        '0.2.0',
        () => Promise.resolve({ ok: false, json: () => Promise.resolve({}) }),
        () =>
          Promise.resolve({
            ok: true,
            url: 'https://github.com/untrusted/lumiere/releases/tag/v9.0.0',
            json: () => Promise.resolve({}),
          }),
      ),
    ).resolves.toEqual({ status: 'failed', currentVersion: '0.2.0' })
  })

  it('uses semantic-version precedence for prereleases', () => {
    expect(compareVersions('0.2.0', '0.2.0-preview.1')).toBe(1)
    expect(compareVersions('0.2.0-preview.2', '0.2.0-preview.10')).toBe(-1)
    expect(compareVersions('1.0.0', '0.99.99')).toBe(1)
  })
})
