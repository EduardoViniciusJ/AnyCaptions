// Guides are the pages people land on from a search: one question each,
// answered in the first paragraph. Same shape in every language, so a
// missing piece is a compile error.
export const guideSlugs = [
  'live-captions-for-games',
  'live-captions-for-calls',
  'translate-videos-in-real-time',
  'offline-voice-dictation',
  'windows-live-captions-alternative',
] as const;
export type GuideSlug = (typeof guideSlugs)[number];

export function isGuideSlug(value: string): value is GuideSlug {
  return (guideSlugs as readonly string[]).includes(value);
}

export interface Question {
  q: string;
  a: string;
}

export interface Guide {
  /** Page title and main heading: the search phrase, in plain words. */
  title: string;
  description: string;
  /** The direct answer, read before anything else. */
  answer: string;
  steps: { heading: string; items: string[] };
  sections: { heading: string; paragraphs: string[] }[];
  /** Side-by-side comparison; the first column names the row. */
  table?: {
    heading: string;
    headers: [string, string, string];
    rows: [string, string, string][];
    source: { before: string; link: string; after: string };
  };
  faq: Question[];
}

export interface GuideMessages {
  /** Link label in the header and footer. */
  label: string;
  index: { title: string; description: string; heading: string; intro: string };
  faqHeading: string;
  download: string;
  docs: string;
  more: string;
  homeFaq: { heading: string; sub: string; items: Question[] };
  guides: Record<GuideSlug, Guide>;
}
