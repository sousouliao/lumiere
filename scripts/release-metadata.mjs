import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { basename, dirname, extname, join, relative, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const changelogPath = join(repositoryRoot, 'CHANGELOG.md')
const desktopPackagePath = join(repositoryRoot, 'apps', 'desktop', 'package.json')
const readmePath = join(repositoryRoot, 'README.md')
const repositoryUrl = 'https://github.com/sousouliao/lumiere'
const semverPattern =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/
const categoryOrder = ['Added', 'Changed', 'Fixed', 'Known limitations']
const allowedCategories = new Set(categoryOrder)
const allowedPlatforms = new Set(['macOS', 'Windows'])

export function parseSemver(value) {
  const match = semverPattern.exec(value)
  if (!match) throw new Error(`Invalid semantic version: ${value}`)
  if (match[4]?.split('.').some((part) => /^0\d+$/.test(part))) {
    throw new Error(`Invalid semantic version: ${value}`)
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4]?.split('.') ?? [],
  }
}

function isValidDate(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false
  const date = new Date(`${value}T00:00:00Z`)
  return !Number.isNaN(date.valueOf()) && date.toISOString().slice(0, 10) === value
}

export function compareSemver(left, right) {
  const a = parseSemver(left)
  const b = parseSemver(right)
  for (const key of ['major', 'minor', 'patch']) {
    if (a[key] !== b[key]) return a[key] < b[key] ? -1 : 1
  }
  if (a.prerelease.length === 0 || b.prerelease.length === 0) {
    return a.prerelease.length === b.prerelease.length ? 0 : a.prerelease.length === 0 ? 1 : -1
  }
  const length = Math.max(a.prerelease.length, b.prerelease.length)
  for (let index = 0; index < length; index += 1) {
    const leftPart = a.prerelease[index]
    const rightPart = b.prerelease[index]
    if (leftPart === undefined || rightPart === undefined) return leftPart === undefined ? -1 : 1
    if (leftPart === rightPart) continue
    const leftNumeric = /^\d+$/.test(leftPart)
    const rightNumeric = /^\d+$/.test(rightPart)
    if (leftNumeric && rightNumeric) return Number(leftPart) < Number(rightPart) ? -1 : 1
    if (leftNumeric !== rightNumeric) return leftNumeric ? -1 : 1
    return leftPart < rightPart ? -1 : 1
  }
  return 0
}

function splitBodyAndLinks(value) {
  const linkStart = value.search(/^\[[^\]]+\]: /m)
  if (linkStart === -1) return { body: value.trim(), links: '' }
  return { body: value.slice(0, linkStart).trim(), links: value.slice(linkStart).trim() }
}

function parsePlatforms(value, context) {
  const platforms = value.split(',').map((platform) => platform.trim())
  if (
    platforms.length === 0 ||
    new Set(platforms).size !== platforms.length ||
    platforms.some((platform) => !allowedPlatforms.has(platform))
  ) {
    throw new Error(`${context} has invalid release platforms: ${value}`)
  }
  const canonical = ['macOS', 'Windows'].filter((platform) => platforms.includes(platform))
  if (platforms.join(', ') !== canonical.join(', ')) {
    throw new Error(
      `${context} release platforms must use canonical order: ${canonical.join(', ')}`,
    )
  }
  return platforms
}

function parseReleaseBody(body, { candidate, context }) {
  const targetMatch = body.match(/^Target version: `([^`]+)`$/m)
  if (candidate && !targetMatch)
    throw new Error('The Unreleased section is missing Target version.')
  if (!candidate && targetMatch) throw new Error(`${context} must not contain Target version.`)
  const platformMatch = body.match(/^Release platforms: (.+)$/m)
  if (!platformMatch) throw new Error(`${context} is missing Release platforms.`)

  const headings = [...body.matchAll(/^### (.+)$/gm)]
  if (headings.length === 0)
    throw new Error(`${context} must contain at least one change category.`)
  const categories = []
  for (let index = 0; index < headings.length; index += 1) {
    const heading = headings[index]
    const name = heading[1]
    if (!allowedCategories.has(name)) throw new Error(`${context} has unknown category: ${name}`)
    if (categories.some((category) => category.name === name)) {
      throw new Error(`${context} contains duplicate category: ${name}`)
    }
    const priorCategory = categories.at(-1)?.name
    if (priorCategory && categoryOrder.indexOf(name) < categoryOrder.indexOf(priorCategory)) {
      throw new Error(`${context} change categories must use canonical order.`)
    }
    const start = heading.index + heading[0].length
    const end = headings[index + 1]?.index ?? body.length
    const content = body.slice(start, end).trim()
    const contentLines = content.split('\n')
    if (
      !contentLines.some((line) => line.startsWith('- ')) ||
      !contentLines.every((line) => line === '' || line.startsWith('- ') || /^\s{2,}\S/.test(line))
    ) {
      throw new Error(`${context} category ${name} must contain Markdown list items only.`)
    }
    categories.push({ name, content })
  }

  const metadataEnd = headings[0].index
  const metadata = body.slice(0, metadataEnd).trim().split(/\n+/).filter(Boolean)
  const expectedMetadata = candidate
    ? [`Target version: \`${targetMatch[1]}\``, `Release platforms: ${platformMatch[1]}`]
    : [`Release platforms: ${platformMatch[1]}`]
  if (metadata.join('\n') !== expectedMetadata.join('\n')) {
    throw new Error(`${context} contains unexpected metadata or prose before its categories.`)
  }

  return {
    targetVersion: targetMatch?.[1],
    platforms: parsePlatforms(platformMatch[1], context),
    categories,
  }
}

export function parseChangelog(source) {
  const normalized = source.replaceAll('\r\n', '\n')
  if (!normalized.startsWith('# Changelog\n'))
    throw new Error('CHANGELOG.md must start with # Changelog.')
  const headers = [...normalized.matchAll(/^## (.+)$/gm)]
  if (headers.length === 0 || headers[0][1] !== '[Unreleased]') {
    throw new Error('CHANGELOG.md must begin with an Unreleased section.')
  }
  if (headers.filter((header) => header[1] === '[Unreleased]').length !== 1) {
    throw new Error('CHANGELOG.md must contain exactly one Unreleased section.')
  }

  const sections = headers.map((header, index) => {
    const start = header.index + header[0].length
    const end = headers[index + 1]?.index ?? normalized.length
    return { header: header[1], body: normalized.slice(start, end) }
  })
  const finalSection = sections.at(-1)
  const { body: finalBody, links } = splitBodyAndLinks(finalSection.body)
  finalSection.body = finalBody

  const unreleasedBody = sections[0].body.trim()
  const candidate = unreleasedBody
    ? parseReleaseBody(unreleasedBody, { candidate: true, context: 'Unreleased' })
    : undefined
  if (candidate) parseSemver(candidate.targetVersion)

  const releases = sections.slice(1).map((section) => {
    const match = /^\[([^\]]+)\] - (\d{4}-\d{2}-\d{2})$/.exec(section.header)
    if (!match) throw new Error(`Invalid release heading: ${section.header}`)
    parseSemver(match[1])
    if (!isValidDate(match[2])) {
      throw new Error(`Invalid release date: ${match[2]}`)
    }
    return {
      version: match[1],
      date: match[2],
      body: section.body.trim(),
      ...parseReleaseBody(section.body.trim(), {
        candidate: false,
        context: `Release ${match[1]}`,
      }),
    }
  })

  const versions = releases.map((release) => release.version)
  if (new Set(versions).size !== versions.length)
    throw new Error('Release versions must be unique.')
  for (let index = 1; index < releases.length; index += 1) {
    if (compareSemver(releases[index - 1].version, releases[index].version) <= 0) {
      throw new Error('Released versions must appear in descending semantic-version order.')
    }
  }
  if (
    candidate &&
    releases[0] &&
    compareSemver(candidate.targetVersion, releases[0].version) <= 0
  ) {
    throw new Error('The target version must be newer than the latest released version.')
  }

  return { candidate, releases, links }
}

export function validateReleaseState({ changelog, packageVersion, publishing = false }) {
  parseSemver(packageVersion)
  const parsed = parseChangelog(changelog)
  if (publishing && parsed.candidate) {
    throw new Error('The Unreleased section must be finalized before publishing.')
  }
  if (parsed.candidate && parsed.candidate.platforms.join(', ') !== 'Windows') {
    throw new Error('New release candidates support Windows only.')
  }
  if (publishing && parsed.releases[0]?.platforms.join(', ') !== 'Windows') {
    throw new Error('The active release lane supports Windows only.')
  }
  const expectedVersion = parsed.candidate?.targetVersion ?? parsed.releases[0]?.version
  if (!expectedVersion) throw new Error('CHANGELOG.md contains no candidate or released version.')
  if (packageVersion !== expectedVersion) {
    throw new Error(
      `Desktop package version ${packageVersion} does not match CHANGELOG version ${expectedVersion}.`,
    )
  }
  return parsed
}

export function finalizeChangelog(source, date) {
  if (!isValidDate(date)) {
    throw new Error(`Invalid release date: ${date}`)
  }
  const parsed = parseChangelog(source)
  if (!parsed.candidate) throw new Error('There is no prepared Unreleased candidate to finalize.')
  const candidate = parsed.candidate
  const candidateBody = [
    `Release platforms: ${candidate.platforms.join(', ')}`,
    '',
    ...candidate.categories.flatMap((category) => [
      `### ${category.name}`,
      '',
      category.content,
      '',
    ]),
  ]
    .join('\n')
    .trim()
  const firstReleaseHeader = source.search(/^## \[[^U][^\]]*\] - /m)
  if (firstReleaseHeader === -1) throw new Error('CHANGELOG.md must contain a previous release.')
  const history = source.slice(firstReleaseHeader).trimEnd()
  const historyWithoutLinks = splitBodyAndLinks(history).body
  const existingLinks = splitBodyAndLinks(history).links
  const previousVersion = parsed.releases[0]?.version
  const newLink = previousVersion
    ? `[${candidate.targetVersion}]: ${repositoryUrl}/compare/v${previousVersion}...v${candidate.targetVersion}`
    : `[${candidate.targetVersion}]: ${repositoryUrl}/releases/tag/v${candidate.targetVersion}`
  const links = [newLink, existingLinks].filter(Boolean).join('\n')
  return [
    '# Changelog',
    '',
    'All notable user-visible changes to Lumiere are documented in this file.',
    '',
    '## [Unreleased]',
    '',
    `## [${candidate.targetVersion}] - ${date}`,
    '',
    candidateBody,
    '',
    historyWithoutLinks.replace(/^.*?## /s, '## '),
    '',
    links,
    '',
  ].join('\n')
}

export function validateReleaseBaseline(parsed, packageVersion, latestStableTag) {
  if (!latestStableTag?.startsWith('v')) {
    throw new Error('The latest stable release must use a vX.Y.Z tag.')
  }
  const latestStable = parsed.releases
    .slice(1)
    .find((release) => parseSemver(release.version).prerelease.length === 0)
  if (!latestStable) throw new Error('CHANGELOG.md has no previous stable release baseline.')
  if (latestStableTag !== `v${latestStable.version}`) {
    throw new Error(
      `Latest stable GitHub Release ${latestStableTag} does not match CHANGELOG baseline v${latestStable.version}.`,
    )
  }
  if (compareSemver(packageVersion, latestStable.version) <= 0) {
    throw new Error('The publishing version must be newer than the latest stable GitHub Release.')
  }
  return latestStable
}

export function validateInstallationGuidance(readme, platforms) {
  for (const platform of platforms) {
    if (!readme.includes(`## Install on ${platform}`)) {
      throw new Error(`README.md is missing installation instructions for ${platform}.`)
    }
  }
}

export function createReleaseNotes(release) {
  const changes = release.categories
    .map((category) => `## ${category.name}\n\n${category.content}`)
    .join('\n\n')
  const downloads = [`- Windows: ` + '`' + `Lumiere-Setup-${release.version}-x64.exe` + '`']

  return [
    changes,
    '',
    '## Downloads',
    '',
    ...downloads,
    '- Integrity: verify the downloaded installer against `SHA256SUMS`.',
    '',
    `See the [versioned installation instructions](${repositoryUrl}/blob/v${release.version}/README.md) before first launch.`,
    '',
  ].join('\n')
}

async function sha256File(filePath) {
  const hash = createHash('sha256')
  for await (const chunk of createReadStream(filePath)) hash.update(chunk)
  return hash.digest('hex')
}

async function findReleaseBinaries(directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) files.push(...(await findReleaseBinaries(path)))
    else if (extname(entry.name) === '.exe') files.push(path)
  }
  return files.sort()
}

export async function createChecksumManifest(directory) {
  const files = await findReleaseBinaries(directory)
  if (files.length === 0) throw new Error(`No release binaries were found under ${directory}.`)
  const names = files.map((file) => basename(file))
  if (new Set(names).size !== names.length) throw new Error('Release binary names must be unique.')
  const lines = []
  for (const file of files) lines.push(`${await sha256File(file)}  ${basename(file)}`)
  return `${lines.join('\n')}\n`
}

function argumentValue(name) {
  const index = process.argv.indexOf(name)
  return index === -1 ? undefined : process.argv[index + 1]
}

async function readRepositoryState({ publishing = false } = {}) {
  const changelog = await readFile(changelogPath, 'utf8')
  const desktopPackage = JSON.parse(await readFile(desktopPackagePath, 'utf8'))
  const tauriConfig = JSON.parse(
    await readFile(join(repositoryRoot, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json'), 'utf8'),
  )
  if (desktopPackage.version !== tauriConfig.version) {
    throw new Error('Desktop package and Tauri versions must agree before release operations.')
  }
  const parsed = validateReleaseState({
    changelog,
    packageVersion: desktopPackage.version,
    publishing,
  })
  const release = parsed.candidate ?? parsed.releases[0]
  validateInstallationGuidance(await readFile(readmePath, 'utf8'), release.platforms)
  return { changelog, desktopPackage, parsed }
}

async function main() {
  const command = process.argv[2]
  if (command === 'check') {
    const { parsed } = await readRepositoryState({
      publishing: process.argv.includes('--publishing'),
    })
    console.log(
      parsed.candidate
        ? `Prepared release metadata is valid for ${parsed.candidate.targetVersion}.`
        : `Release metadata is valid for ${parsed.releases[0].version}.`,
    )
    return
  }
  if (command === 'finalize') {
    const date = argumentValue('--date')
    if (!date) throw new Error('finalize requires --date YYYY-MM-DD.')
    const { changelog, desktopPackage } = await readRepositoryState()
    const finalized = finalizeChangelog(changelog, date)
    validateReleaseState({
      changelog: finalized,
      packageVersion: desktopPackage.version,
      publishing: true,
    })
    await writeFile(changelogPath, finalized, 'utf8')
    console.log(`Finalized CHANGELOG.md for ${desktopPackage.version}.`)
    return
  }
  if (command === 'inspect' || command === 'notes') {
    const { parsed, desktopPackage } = await readRepositoryState({ publishing: true })
    const release = parsed.releases[0]
    if (release.version !== desktopPackage.version)
      throw new Error('Latest release metadata mismatch.')
    if (command === 'inspect') {
      const outputPath = argumentValue('--github-output')
      const result = {
        version: release.version,
        platforms: release.platforms,
        windows: release.platforms.includes('Windows'),
        prerelease: parseSemver(release.version).prerelease.length > 0,
      }
      if (outputPath) {
        await writeFile(
          outputPath,
          `version=${result.version}\nwindows=${String(result.windows)}\nprerelease=${String(result.prerelease)}\n`,
          { encoding: 'utf8', flag: 'a' },
        )
      } else console.log(JSON.stringify(result))
      return
    }
    const outputPath = argumentValue('--output')
    if (!outputPath) throw new Error('notes requires --output PATH.')
    await mkdir(dirname(resolve(outputPath)), { recursive: true })
    await writeFile(resolve(outputPath), createReleaseNotes(release), 'utf8')
    console.log(`Wrote release notes to ${relative(repositoryRoot, resolve(outputPath))}.`)
    return
  }
  if (command === 'baseline') {
    const latestTag = argumentValue('--latest-tag')
    if (!latestTag?.startsWith('v')) throw new Error('baseline requires --latest-tag vX.Y.Z.')
    const { parsed, desktopPackage } = await readRepositoryState({ publishing: true })
    validateReleaseBaseline(parsed, desktopPackage.version, latestTag)
    console.log(`Release baseline ${latestTag} is valid for ${desktopPackage.version}.`)
    return
  }
  if (command === 'checksums') {
    const directory = argumentValue('--directory')
    const outputPath = argumentValue('--output')
    if (!directory || !outputPath) throw new Error('checksums requires --directory and --output.')
    const manifest = await createChecksumManifest(resolve(directory))
    await writeFile(resolve(outputPath), manifest, 'utf8')
    console.log(`Wrote ${relative(repositoryRoot, resolve(outputPath))}.`)
    return
  }
  throw new Error('Usage: release-metadata.mjs <check|finalize|inspect|notes|baseline|checksums>')
}

const invokedUrl = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : undefined
if (import.meta.url === invokedUrl) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
