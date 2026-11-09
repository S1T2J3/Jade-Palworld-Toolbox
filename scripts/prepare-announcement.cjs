const fs = require('node:fs');
const path = require('node:path');

const project = path.resolve(__dirname, '..');
const source = fs.readFileSync(path.join(project, 'work/build.cjs'), 'utf8');
const version = /const releaseVersion = '([^']+)'/.exec(source)?.[1];
if (!version) throw new Error('Cannot read the build version');

const filename = path.join(project, 'announcements.json');
const feed = JSON.parse(fs.readFileSync(filename, 'utf8'));
const notice = feed.announcements.find(row => row.id === 'release-v' + version);
if (notice && notice.popup !== true) {
  notice.popup = true;
  fs.writeFileSync(filename, JSON.stringify(feed, null, 2) + '\n', 'utf8');
}
