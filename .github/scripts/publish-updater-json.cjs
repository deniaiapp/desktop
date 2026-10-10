// Builds the Tauri updater's latest.json from a release's signed assets and
// uploads it. Runs once after every build job has finished: letting each
// parallel job merge its own entries (tauri-action's default) races, and the
// last writer can drop other platforms (v0.3.1 shipped with macOS arm64 only).

const ARCH = {
  aarch64: "aarch64",
  arm64: "aarch64",
  x64: "x86_64",
  amd64: "x86_64",
  x86_64: "x86_64",
};

// Asset names follow "[name]_[version]_[platform]_[arch][setup][ext]".
// Bare "<os>-<arch>" keys mirror tauri-action: AppImage on Linux, MSI on Windows.
const RULES = [
  { pattern: /_darwin_(\w+)\.app\.tar\.gz$/, keys: ["darwin-{arch}", "darwin-{arch}-app"] },
  { pattern: /_linux_(\w+)\.AppImage$/, keys: ["linux-{arch}", "linux-{arch}-appimage"] },
  { pattern: /_linux_(\w+)\.deb$/, keys: ["linux-{arch}-deb"] },
  { pattern: /_linux_(\w+)\.rpm$/, keys: ["linux-{arch}-rpm"] },
  { pattern: /_windows_(\w+)\.msi$/, keys: ["windows-{arch}", "windows-{arch}-msi"] },
  { pattern: /_windows_(\w+)-setup\.exe$/, keys: ["windows-{arch}-nsis"] },
];

async function buildPlatforms({ github, core, owner, repo, releaseId }) {
  const assets = await github.paginate(github.rest.repos.listReleaseAssets, {
    owner,
    repo,
    release_id: releaseId,
    per_page: 100,
  });
  const byName = new Map(assets.map((asset) => [asset.name, asset]));
  const platforms = {};

  for (const asset of assets) {
    for (const rule of RULES) {
      const match = asset.name.match(rule.pattern);
      if (!match) {
        continue;
      }

      const arch = ARCH[match[1]];
      const sigAsset = byName.get(`${asset.name}.sig`);
      if (!arch || !sigAsset) {
        core.warning(`Skipping ${asset.name}: unknown arch or missing .sig`);
        continue;
      }

      const { data } = await github.rest.repos.getReleaseAsset({
        owner,
        repo,
        asset_id: sigAsset.id,
        headers: { accept: "application/octet-stream" },
      });
      const signature = Buffer.from(data).toString("utf8").trim();

      for (const key of rule.keys) {
        platforms[key.replace("{arch}", arch)] = { signature, url: asset.url };
      }
    }
  }

  return { assets, platforms };
}

async function publishUpdaterJson({ github, context, core, releaseId, version }) {
  const { owner, repo } = context.repo;
  const { assets, platforms } = await buildPlatforms({ github, core, owner, repo, releaseId });

  const keys = Object.keys(platforms).sort();
  if (keys.length === 0) {
    core.setFailed("No signed updater bundles found; not publishing latest.json");
    return;
  }

  const latest = {
    version,
    notes: "",
    pub_date: new Date().toISOString(),
    platforms: Object.fromEntries(keys.map((key) => [key, platforms[key]])),
  };

  const existing = assets.find((asset) => asset.name === "latest.json");
  if (existing) {
    await github.rest.repos.deleteReleaseAsset({ owner, repo, asset_id: existing.id });
  }

  await github.rest.repos.uploadReleaseAsset({
    owner,
    repo,
    release_id: releaseId,
    name: "latest.json",
    data: JSON.stringify(latest, null, 2),
    headers: { "content-type": "application/json" },
  });

  core.info(`Published latest.json for ${version}: ${keys.join(", ")}`);
}

module.exports = publishUpdaterJson;
module.exports.buildPlatforms = buildPlatforms;
