// ESM run: `NODE_OPTIONS=--experimental-vm-modules nubx jest -c jest.esm.config.cjs`.
module.exports = {
  watchman: false,
  testPathIgnorePatterns: ["/node_modules/"],
  transform: { "\\.sql$": "reylo/jest" },
  extensionsToTreatAsEsm: [".sql"],
};
