import type { APIRoute } from 'astro';
import { htmlLang, langs, localePath } from '../i18n';
import { guideSlugs } from '../i18n/guides';
import { getLatestRelease } from '../lib/api/releases';

const site = 'https://anycaptions.app';
// Day the written pages last changed; update it when their text changes.
const contentDate = '2026-11-17';

// Every public page in every language, each one pointing to its
// translations, so search engines index the six versions as one page.
export const GET: APIRoute = async () => {
  const release = await getLatestRelease();
  const releaseDate = release?.publishedAt?.slice(0, 10);
  const newest = releaseDate && releaseDate > contentDate ? releaseDate : contentDate;
  const pages = [
    { path: '/', lastmod: newest },
    { path: '/docs', lastmod: contentDate },
    { path: '/changelog', lastmod: releaseDate ?? contentDate },
    { path: '/contribute', lastmod: contentDate },
    { path: '/guides', lastmod: contentDate },
    ...guideSlugs.map(slug => ({ path: `/guides/${slug}`, lastmod: contentDate })),
  ];
  const urls = pages.flatMap(page => langs.map(lang => {
    const alternates = langs
      .map(other => `<xhtml:link rel="alternate" hreflang="${htmlLang[other]}" href="${site}${localePath(other, page.path)}"/>`)
      .join('');
    return `<url><loc>${site}${localePath(lang, page.path)}</loc><lastmod>${page.lastmod}</lastmod>${alternates}<xhtml:link rel="alternate" hreflang="x-default" href="${site}${page.path}"/></url>`;
  }));
  const body = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${urls.join('\n')}\n</urlset>\n`;
  return new Response(body, { headers: { 'Content-Type': 'application/xml; charset=utf-8', 'Cache-Control': 'public, max-age=3600' } });
};
