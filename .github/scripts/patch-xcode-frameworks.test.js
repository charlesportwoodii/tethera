const test = require('node:test');
const assert = require('node:assert');
const { XcodeFrameworks } = require('./patch-xcode-frameworks');

test('adds OTHER_LDFLAGS to a block that has none', () => {
  const pbx = 'buildSettings = {\n\t\t\t\tSDKROOT = iphoneos;\n\t\t\t}';
  const { pbx: out, blocks, changed } = XcodeFrameworks.patch(pbx);

  assert.strictEqual(blocks, 1);
  assert.strictEqual(changed, 1);
  assert.match(out, /OTHER_LDFLAGS = "\$\(inherited\) -framework SystemConfiguration";/);
});

test('appends to an existing list as separate elements', () => {
  const pbx =
    'buildSettings = {\n\t\t\t\tOTHER_LDFLAGS = (\n\t\t\t\t\t"-ObjC",\n\t\t\t\t);\n\t\t\t}';
  const { pbx: out, changed } = XcodeFrameworks.patch(pbx);

  assert.strictEqual(changed, 1);
  assert.match(out, /"-ObjC",/);
  assert.match(out, /"-framework",\n\t+"SystemConfiguration",/);
  // One element per flag. Xcode does not split a list element, so a combined
  // "-framework SystemConfiguration" would reach ld as one unknown argument.
  assert.doesNotMatch(out, /"-framework SystemConfiguration"/);
});

test('appends inside the quotes of an existing string', () => {
  const pbx = 'buildSettings = {\n\t\t\t\tOTHER_LDFLAGS = "-ObjC";\n\t\t\t}';
  const { pbx: out, changed } = XcodeFrameworks.patch(pbx);

  assert.strictEqual(changed, 1);
  assert.match(out, /OTHER_LDFLAGS = "-ObjC -framework SystemConfiguration";/);
});

test('is idempotent', () => {
  const pbx = 'buildSettings = {\n\t\t\t\tSDKROOT = iphoneos;\n\t\t\t}';
  const once = XcodeFrameworks.patch(pbx).pbx;
  const twice = XcodeFrameworks.patch(once);

  assert.strictEqual(twice.changed, 0);
  assert.strictEqual(twice.pbx, once);
});

test('patches every block, not only the first', () => {
  const one = 'buildSettings = {\n\t\t\t\tSDKROOT = iphoneos;\n\t\t\t}';
  const { blocks, changed } = XcodeFrameworks.patch(`${one}\nfoo\n${one}`);

  assert.strictEqual(blocks, 2);
  assert.strictEqual(changed, 2);
});

test('reports zero blocks when the project layout no longer matches', () => {
  const { blocks, changed } = XcodeFrameworks.patch('/* nothing here */');

  assert.strictEqual(blocks, 0);
  assert.strictEqual(changed, 0);
});

test('the rewritten block still matches the signing script regex', () => {
  const pbx = 'buildSettings = {\n\t\t\t\tCODE_SIGN_STYLE = Automatic;\n\t\t\t}';
  const out = XcodeFrameworks.patch(pbx).pbx;

  // ios-manual-signing.mjs runs after this one and matches with the same
  // negated class. A flag value it cannot step over silently leaves a target
  // on Automatic signing, which fails at export rather than here.
  assert.match(out, /buildSettings = \{([^}]*)\}/);
});
