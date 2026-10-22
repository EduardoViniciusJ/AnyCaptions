export interface Release {
  version: string;
  releaseNotes: string;
  windowsInstallerUrl: string;
  windowsZipUrl: string | null;
  sha256: string;
  publishedAt: string;
}

const internalApi = (process.env.API_INTERNAL_URL || 'http://localhost:5080').replace(/\/$/, '');

export async function getLatestRelease(): Promise<Release | null> {
  try {
    const response = await fetch(`${internalApi}/api/releases/latest`, { cache: 'no-store' });
    return response.ok ? await response.json() as Release : null;
  } catch {
    return null;
  }
}

export async function getReleases(): Promise<Release[]> {
  try {
    const response = await fetch(`${internalApi}/api/releases`, { cache: 'no-store' });
    return response.ok ? await response.json() as Release[] : [];
  } catch {
    return [];
  }
}
