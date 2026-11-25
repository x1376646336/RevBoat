const { readVersions } = require('./validate-prerelease.cjs')

function validateRelease(tag, versions) {
  if (!/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(tag || '')) {
    const current = versions['package.json']
    const hint = /^\d+\.\d+\.\d+$/.test(current || '')
      ? ` Current package.json version is ${current}; use v${current} when all package versions match.`
      : ''
    throw new Error(`Invalid stable release tag ${JSON.stringify(tag ?? '(missing)')}. Use vX.Y.Z; date-only tags such as v20261125 are not supported.${hint}`)
  }
  const version = tag.slice(1)
  for (const [file, actual] of Object.entries(versions)) {
    if (actual !== version) throw new Error(`${file}: expected ${version}, found ${actual}`)
  }
  return version
}

if (require.main === module) {
  try {
    const version = validateRelease(process.env.RELEASE_TAG, readVersions())
    console.log(`Validated stable release ${version}`)
  } catch (error) {
    console.error(`Release validation failed: ${error.message}`)
    process.exitCode = 1
  }
}

module.exports = { validateRelease }
