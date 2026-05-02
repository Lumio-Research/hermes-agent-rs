const provider = process.env.HERMES_UPDATE_PROVIDER || "github";

function resolvePublishConfig() {
  if (provider === "s3") {
    return [
      {
        provider: "s3",
        bucket: process.env.HERMES_S3_BUCKET,
        region: process.env.HERMES_S3_REGION,
        path: process.env.HERMES_S3_PATH || "desktop-updates",
      },
    ];
  }
  return [
    {
      provider: "github",
      owner: process.env.HERMES_GH_OWNER,
      repo: process.env.HERMES_GH_REPO,
      private: process.env.HERMES_GH_PRIVATE === "1",
      releaseType: process.env.HERMES_GH_RELEASE_TYPE || "release",
    },
  ];
}

module.exports = {
  appId: "com.hermes.desktop",
  productName: "Hermes Desktop",
  directories: {
    output: "release",
  },
  files: ["electron/dist/**", "dist/**"],
  mac: {
    category: "public.app-category.developer-tools",
    target: ["dmg", "zip"],
  },
  win: {
    target: ["nsis", "zip"],
  },
  linux: {
    target: ["AppImage", "deb", "rpm"],
    category: "Development",
  },
  publish: resolvePublishConfig(),
};
