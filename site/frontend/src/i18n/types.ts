// Shape shared by every locale file, so a missing key in any language is a
// compile error instead of a blank spot on the page.
/** Vídeo curto com texto ao lado, na home. */
type Spotlight = { title: string; text: string; label: string };
type Feature = { title: string; sub: string; alt: string; items: { title: string; text: string }[] };

export interface LinkedSentence {
  before: string;
  link: string;
  after: string;
}

export interface SiteMessages {
  layout: {
    skipLink: string;
    footer: {
      product: string; legal: string;
      changelog: string; docs: string;
      privacy: string; licenses: string;
    };
  };
  nav: { docs: string; changelog: string; signIn: string; language: string };
  home: {
    title: string;
    description: string;
    jsonLd: string;
    heroTitle: string;
    heroLines: { lead: string; rest: string }[];
    download: string;
    downloadSignInHint: string;
    downloadUnavailable: string;
    videos: {
      amongUs: string; youtube: string; game: string;
      prev: string; next: string; play: string; goTo: string;
    };
    demos: { config: Spotlight; dictation: Spotlight };
    features: [Feature, Feature];
    github: { title: string; sub: string; repo: string; issues: string; copy: string; copied: string };
  };
  docs: {
    title: string;
    description: string;
    kicker: string;
    heading: string;
    intro: string;
    sidebarTitle: string;
    nav: {
      install: string;
      firstSteps: string;
      audio: string;
      overlay: string;
      models: string;
      privacy: string;
      licenses: string;
      trouble: string;
    };
    install: {
      heading: string;
      step1: LinkedSentence;
      step2: LinkedSentence;
      step3: string;
      requirements: string;
    };
    firstSteps: { heading: string; steps: string[] };
    audio: { heading: string; body: string };
    overlay: { heading: string; intro: string; shortcut: string; after: string };
    models: {
      heading: string;
      intro: string;
      items: { name: string; desc: string }[];
      summary: string;
    };
    privacy: {
      heading: string;
      appLead: string;
      appBody: string;
      siteLead: string;
      siteBody: string;
      deleteNote: LinkedSentence;
    };
    licenses: {
      heading: string;
      intro: string;
      nvidiaName: string;
      nvidia: LinkedSentence;
      opusName: string;
      opus: LinkedSentence;
      code: LinkedSentence;
    };
    trouble: {
      heading: string;
      items: { lead: string; rest: string }[];
      last: LinkedSentence;
    };
  };
  account: {
    title: string;
    description: string;
    kicker: string;
    welcome: string;
    profile: string;
    checking: string;
    noEmail: string;
    news: string;
    download: string;
    signOut: string;
    signingOut: string;
    deleteLink: string;
    deleteWarning: string;
    deleteConfirm: string;
    deleting: string;
    cancel: string;
    signInWith: string;
    errSignIn: string;
    errSignOut: string;
    errDelete: string;
    consent: LinkedSentence;
  };
  contribute: {
    title: string;
    description: string;
    heading: string;
    sub: string;
    issueButton: string;
    repoHeading: string;
    repoButton: string;
  };
  changelog: {
    title: string;
    description: string;
    kicker: string;
    heading: string;
    sub: string;
    listLabel: string;
    empty: string;
    versionTag: string;
  };
}
