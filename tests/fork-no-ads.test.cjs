// Fork: no ads. The backend never fetches sponsor spots or partner store cards, strips sponsor
// bookings from Home's banners, and Settings has no "Show sponsored content" switch to offer.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const read = (p) => fs.readFileSync(path.join(__dirname, '..', p), 'utf8');
const cloud = read('src-tauri/src/cloud.rs');
const lib = read('src-tauri/src/lib.rs');

test('the no-ads switch is on', () => {
  assert.match(cloud, /pub const NO_ADS: bool = true;/);
});

test('sponsor spots, their views and Home banner bookings are turned off', () => {
  assert.match(cloud, /pub async fn spons_mine\(app: AppHandle\) -> Res<Value> \{\n    if NO_ADS \{\n        return Ok\(Value::Null\);/);
  assert.match(cloud, /pub async fn spons_seen\(app: AppHandle, id: u64\) -> Res<\(\)> \{\n    if NO_ADS \{\n        return Ok\(\(\)\);/);
  assert.match(cloud, /return Ok\(no_ad_banners\(v\)\);/);
  assert.match(cloud, /\.filter\(home_ok\)\.map\(no_ad_banners\)/);
  assert.match(cloud, /o\.remove\("sp"\);/);
});

test('partner store cards are turned off', () => {
  assert.match(lib, /async fn ig_info\(st: State<'_, App>\) -> Res<Value> \{\n    if cloud::NO_ADS \{\n        return Ok\(Value::Null\);/);
  assert.match(lib, /async fn ig_view\([^)]*\) -> Res<\(\)> \{\n    if cloud::NO_ADS \{\n        return Ok\(\(\)\);/);
});

test('Settings has no sponsored content switch, on the desktop or in Big Picture', () => {
  assert.doesNotMatch(read('ui/app.js'), /row\(t\('Show sponsored content'\)/);
  assert.doesNotMatch(read('ui/tv.js'), /data-key="s:sponsored"/);
});
