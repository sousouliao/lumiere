import { mkdir, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import sharp from 'sharp'

const repositoryRoot = resolve(import.meta.dirname, '..')
const sourcePath = join(repositoryRoot, 'assets/brand/lumiere-logo.png')
const iconsRoot = join(repositoryRoot, 'apps/desktop/resources/icons')
const windowsRoot = join(iconsRoot, 'windows')

const windowsAppSizes = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256]
const windowsTraySizes = [16, 20, 24, 32, 40, 48, 64]

async function createAppArtwork() {
  const { data, info } = await sharp(sourcePath)
    .removeAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true })
  const cutout = Buffer.alloc(info.width * info.height * 4)

  for (let sourceOffset = 0, outputOffset = 0; sourceOffset < data.length; sourceOffset += 3) {
    const red = data[sourceOffset]
    const green = data[sourceOffset + 1]
    const blue = data[sourceOffset + 2]
    const coralSeparation = Math.min(red - green, red - blue)
    const redScore = Math.max(0, Math.min(1, (red - 135) / 85))
    const coralScore = Math.max(0, Math.min(1, (coralSeparation - 25) / 65))
    const alpha = Math.round(255 * (1 - redScore * coralScore))

    cutout[outputOffset] = red
    cutout[outputOffset + 1] = green
    cutout[outputOffset + 2] = blue
    cutout[outputOffset + 3] = alpha
    outputOffset += 4
  }

  const canvasSize = 1024
  const characterSize = canvasSize
  const characterLeft = Math.round((canvasSize - characterSize) / 2)
  const character = await sharp(cutout, {
    raw: { width: info.width, height: info.height, channels: 4 },
  })
    .resize(characterSize, characterSize, { fit: 'fill', kernel: sharp.kernel.lanczos3 })
    .png()
    .toBuffer()

  return sharp({
    create: { width: canvasSize, height: canvasSize, channels: 4, background: '#E8665A' },
  })
    .composite([
      {
        input: character,
        left: characterLeft,
        top: canvasSize - characterSize,
      },
    ])
    .png()
    .toBuffer()
}

async function createTemplateMaster() {
  const { data, info } = await sharp(sourcePath)
    .removeAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true })
  const output = Buffer.alloc(info.width * info.height * 4)

  for (let sourceOffset = 0, outputOffset = 0; sourceOffset < data.length; sourceOffset += 3) {
    const red = data[sourceOffset]
    const green = data[sourceOffset + 1]
    const blue = data[sourceOffset + 2]
    const lowestChannel = Math.min(red, green, blue)
    const channelSpread = Math.max(red, green, blue) - lowestChannel
    const lightnessAlpha = Math.max(0, Math.min(255, (lowestChannel - 118) * 3.2))
    const neutralityAlpha = Math.max(0, Math.min(255, (94 - channelSpread) * 4))
    const alpha = Math.round((lightnessAlpha * neutralityAlpha) / 255)

    output[outputOffset] = 0
    output[outputOffset + 1] = 0
    output[outputOffset + 2] = 0
    output[outputOffset + 3] = alpha
    outputOffset += 4
  }

  return sharp(output, {
    raw: { width: info.width, height: info.height, channels: 4 },
  })
    .blur(0.6)
    .png()
    .toBuffer()
}

async function writeResizedPng(input, size, outputPath, density) {
  await sharp(input)
    .resize(size, size, { fit: 'fill', kernel: sharp.kernel.lanczos3 })
    .png({ compressionLevel: 9, palette: false })
    .withMetadata(density ? { density } : {})
    .toFile(outputPath)
}

function createIco(pngs) {
  const headerSize = 6
  const directoryEntrySize = 16
  let dataOffset = headerSize + pngs.length * directoryEntrySize
  const header = Buffer.alloc(headerSize)
  header.writeUInt16LE(0, 0)
  header.writeUInt16LE(1, 2)
  header.writeUInt16LE(pngs.length, 4)

  const entries = pngs.map(({ size, buffer }) => {
    const entry = Buffer.alloc(directoryEntrySize)
    entry.writeUInt8(size === 256 ? 0 : size, 0)
    entry.writeUInt8(size === 256 ? 0 : size, 1)
    entry.writeUInt8(0, 2)
    entry.writeUInt8(0, 3)
    entry.writeUInt16LE(1, 4)
    entry.writeUInt16LE(32, 6)
    entry.writeUInt32LE(buffer.length, 8)
    entry.writeUInt32LE(dataOffset, 12)
    dataOffset += buffer.length
    return entry
  })

  return Buffer.concat([header, ...entries, ...pngs.map(({ buffer }) => buffer)])
}

async function createIcoFromInput(input, sizes, outputPath) {
  const pngs = await Promise.all(
    sizes.map(async (size) => ({
      size,
      buffer: await sharp(input)
        .resize(size, size, { fit: 'fill', kernel: sharp.kernel.lanczos3 })
        .png({ compressionLevel: 9, palette: false })
        .toBuffer(),
    })),
  )
  await writeFile(outputPath, createIco(pngs))
}

async function main() {
  await mkdir(windowsRoot, { recursive: true })

  const sourceMetadata = await sharp(sourcePath).metadata()
  if (sourceMetadata.width !== sourceMetadata.height || (sourceMetadata.width ?? 0) < 1024) {
    throw new Error('The canonical logo must be a square image at least 1024 pixels wide.')
  }

  const appArtwork = await createAppArtwork()
  const templateMaster = await createTemplateMaster()
  await writeResizedPng(appArtwork, 256, join(windowsRoot, 'app.png'))

  const windowsTrayMaster = await sharp(templateMaster).tint('#E8665A').png().toBuffer()
  await Promise.all([
    writeResizedPng(windowsTrayMaster, 32, join(windowsRoot, 'tray.png')),
    createIcoFromInput(appArtwork, windowsAppSizes, join(windowsRoot, 'app.ico')),
    createIcoFromInput(windowsTrayMaster, windowsTraySizes, join(windowsRoot, 'tray.ico')),
  ])

  console.log(`Generated desktop icon assets from ${sourcePath}`)
}

await main()
