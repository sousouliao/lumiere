import { createHash } from 'node:crypto'
import { readFile, stat, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const releaseRoot = join(repositoryRoot, 'artifacts', 'windows', 'release')

async function main() {
  const desktopPackage = JSON.parse(
    await readFile(join(repositoryRoot, 'apps', 'desktop', 'package.json'), 'utf8'),
  )
  const fileName = `Lumiere-Setup-${desktopPackage.version}-x64.exe`
  const installerPath = join(releaseRoot, fileName)
  const bytes = await readFile(installerPath)
  const fileStat = await stat(installerPath)
  const sha512 = createHash('sha512').update(bytes).digest('base64')
  const sha256 = createHash('sha256').update(bytes).digest('hex')
  const latest = [
    `version: ${desktopPackage.version}`,
    'files:',
    `  - url: ${fileName}`,
    `    sha512: ${sha512}`,
    `    size: ${fileStat.size}`,
    `path: ${fileName}`,
    `sha512: ${sha512}`,
    `releaseDate: '${new Date().toISOString()}'`,
    '',
  ].join('\n')
  const signature = (await readFile(`${installerPath}.sig`, 'utf8')).trim()
  const tauriMetadata = {
    version: desktopPackage.version,
    pub_date: new Date().toISOString(),
    platforms: {
      'windows-x86_64': {
        signature,
        url: `https://github.com/sousouliao/lumiere/releases/download/v${desktopPackage.version}/${fileName}`,
      },
    },
  }
  await writeFile(
    join(releaseRoot, 'latest.json'),
    `${JSON.stringify(tauriMetadata, null, 2)}\n`,
    'utf8',
  )
  await writeFile(join(releaseRoot, 'latest.yml'), latest, 'utf8')
  await writeFile(join(releaseRoot, 'SHA256SUMS'), `${sha256}  ${fileName}\n`, 'utf8')
}

const invokedUrl = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : undefined
if (import.meta.url === invokedUrl) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
