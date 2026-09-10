import { readFile, readdir } from 'node:fs/promises'
import { resolve } from 'node:path'
import Ajv2020 from 'ajv/dist/2020.js'
import { describe, expect, it } from 'vitest'

const protocolDirectory = resolve(process.cwd(), '../../protocol/platform-host')

describe.each([1, 2, 3, 4, 5])('platform host protocol v%i', (version) => {
  it('keeps every checked-in fixture conformant with its language-neutral schema', async () => {
    const validate = await validatorFor(version)
    const fixtureDirectory = `${protocolDirectory}/fixtures/v${String(version)}`
    const fixtureNames = (await readdir(fixtureDirectory))
      .filter((name) => name.endsWith('.json'))
      .sort()

    expect(fixtureNames.length).toBeGreaterThanOrEqual(6)

    for (const fixtureName of fixtureNames) {
      const fixture = JSON.parse(
        await readFile(`${fixtureDirectory}/${fixtureName}`, 'utf8'),
      ) as unknown

      expect(validate(fixture), `${fixtureName}: ${JSON.stringify(validate.errors)}`).toBe(true)
    }
  })

  it('rejects protocol messages with unknown fields', async () => {
    const validate = await validatorFor(version)

    expect(
      validate(
        version >= 3
          ? {
              version,
              id: 'invalid-1',
              method: 'captureDisplay',
              params: { delivery: 'folder', rawFrame: true },
            }
          : {
              version,
              id: 'invalid-1',
              method: 'capture',
              params: { mode: 'display', delivery: 'folder', rawFrame: true },
            },
      ),
    ).toBe(false)
  })
})

async function validatorFor(version: number): Promise<ReturnType<Ajv2020['compile']>> {
  const ajv = new Ajv2020({ allErrors: true, strict: true })
  if (version === 5) {
    const priorSchema = JSON.parse(
      await readFile(`${protocolDirectory}/v4.schema.json`, 'utf8'),
    ) as object
    ajv.addSchema(priorSchema)
  }
  const schema = JSON.parse(
    await readFile(`${protocolDirectory}/v${String(version)}.schema.json`, 'utf8'),
  ) as object
  return ajv.compile(schema)
}
