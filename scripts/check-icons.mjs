import { readFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import sharp from 'sharp'

const repositoryRoot = resolve(import.meta.dirname, '..')
const iconsRoot = join(repositoryRoot, 'apps/desktop/resources/icons')

function assert(condition, message) {
  if (!condition) {
    throw new Error(message)
  }
}

async function assertPng(
  path,
  expectedWidth,
  expectedHeight,
  expectedDensity,
  requiresAlpha = true,
) {
  const metadata = await sharp(path).metadata()
  assert(metadata.format === 'png', `${path} must be a PNG.`)
  assert(
    metadata.width === expectedWidth && metadata.height === expectedHeight,
    `${path} must be ${expectedWidth} × ${expectedHeight}.`,
  )
  if (requiresAlpha) {
    assert(metadata.hasAlpha, `${path} must contain an alpha channel.`)
  }
  if (expectedDensity) {
    assert(
      metadata.density === expectedDensity,
      `${path} must use ${expectedDensity} DPI metadata.`,
    )
  }
}

async function icoSizes(path) {
  const buffer = await readFile(path)
  assert(buffer.readUInt16LE(0) === 0 && buffer.readUInt16LE(2) === 1, `${path} is not an ICO.`)
  const count = buffer.readUInt16LE(4)
  return Array.from({ length: count }, (_, index) => {
    const offset = 6 + index * 16
    const width = buffer.readUInt8(offset)
    const height = buffer.readUInt8(offset + 1)
    assert(width === height, `${path} contains a non-square representation.`)
    return width === 0 ? 256 : width
  })
}

await Promise.all([
  assertPng(join(iconsRoot, 'windows/app.png'), 256, 256, undefined, false),
  assertPng(join(iconsRoot, 'windows/tray.png'), 32, 32),
])

const [appIcoSizes, trayIcoSizes] = await Promise.all([
  icoSizes(join(iconsRoot, 'windows/app.ico')),
  icoSizes(join(iconsRoot, 'windows/tray.ico')),
])

assert(
  appIcoSizes.join(',') === '16,20,24,30,32,36,40,48,60,64,72,80,96,256',
  'The Windows application ICO does not contain the required DPI representations.',
)
assert(
  trayIcoSizes.join(',') === '16,20,24,32,40,48,64',
  'The Windows tray ICO does not contain the required DPI representations.',
)
console.log('Desktop icon assets are complete and internally valid.')
