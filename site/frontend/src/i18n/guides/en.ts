import type { GuideMessages } from './types';

export const en: GuideMessages = {
  label: 'Guides',
  index: {
    title: 'Guides',
    description: 'How to get live captions, real-time translation and offline voice dictation on Windows with AnyCaptions.',
    heading: 'Guides',
    intro: 'Short answers to what people ask most: captions in games and calls, translating videos while they play, and typing with your voice.',
  },
  faqHeading: 'Common questions',
  download: 'Download for Windows',
  docs: 'Read the user guide',
  more: 'More guides',
  homeFaq: {
    heading: 'Common questions',
    sub: 'What people ask before installing.',
    items: [
      { q: 'What is AnyCaptions?', a: 'AnyCaptions is a free Windows 11 app that shows live captions for any audio playing on the computer: games, videos, calls and streams. It can translate the captions while you listen and type what you say into any app. Speech recognition and translation run on your own computer.' },
      { q: 'Is AnyCaptions free?', a: 'Yes. The app is free, with no paid plan and no ads, and its code is public on GitHub. You only sign in with Google, GitHub or Discord on the website to download the installer.' },
      { q: 'Does it work offline?', a: 'Yes. After installing, captions, translation and dictation work with no internet connection. The app only uses the network when you ask it to download an extra model or to check for updates.' },
      { q: 'Is my audio sent to a server?', a: 'No. The audio and the captions stay in the memory of your computer. Nothing is uploaded, there is no history and no telemetry.' },
      { q: 'Which languages can it translate?', a: 'English and Portuguese work right after installing. The M2M100 model (491 MB, runs on the processor) translates between 34 languages, and the Riva model (2.8 GB, needs a graphics card) translates into 37. Both are downloaded inside the app.' },
      { q: 'Do I need a graphics card?', a: 'No. Speech recognition uses the graphics card when there is one and falls back to the processor when there is not. Translation with OPUS-MT and M2M100 runs on the processor. Only the Riva model needs a graphics card, with about 3 GB of free video memory.' },
      { q: 'Does it work over games?', a: 'Yes, in windowed or borderless window mode. The caption is a transparent window that stays on top, and once locked, clicks pass through it. Exclusive fullscreen can hide it.' },
      { q: 'How is it different from Windows live captions?', a: 'Windows live captions translate only on Copilot+ PCs, and only into English and Simplified Chinese. AnyCaptions translates on any Windows 11 x64 computer, into up to 37 languages, and also includes voice dictation and optional speaker labels.' },
    ],
  },
  guides: {
    'live-captions-for-games': {
      title: 'Live captions for games on Windows',
      description: 'Show live captions and translate voice chat and cutscenes in any PC game with AnyCaptions. Free, offline, with an overlay that does not block clicks.',
      answer: 'AnyCaptions adds live captions to any PC game on Windows 11. It listens to the sound that already goes to your headphones, so voice chat, cutscenes and streams become text in a transparent overlay on top of the game. It can translate the captions as they appear. It is free and runs on your own computer.',
      steps: {
        heading: 'How to turn on captions in a game',
        items: [
          'Install AnyCaptions and open it.',
          'In the settings, pick the audio output you play with (your headphones or speakers).',
          'Set the game to windowed or borderless window mode.',
          'Click Start, drag the caption where you want it and press Ctrl + Alt + L to lock it.',
        ],
      },
      sections: [
        {
          heading: 'It does not get in the way of the game',
          paragraphs: [
            'When the caption is locked, the controls disappear and clicks pass straight through it, so the mouse keeps working in the game. Press Ctrl + Alt + L again to move or resize it.',
            'AnyCaptions does not read or change the game. It only listens to the audio output of Windows, the same sound you hear.',
          ],
        },
        {
          heading: 'Translate teammates and cutscenes',
          paragraphs: [
            'Choose a translation language and the captions come out translated. English and Portuguese work right after installing. For other languages, download M2M100 (491 MB, 34 languages, runs on the processor) inside the app.',
            'The Riva model translates into 37 languages but uses about 3 GB of video memory and can lower the frame rate. For games, M2M100 or OPUS-MT are the safer choice because they leave the graphics card alone.',
          ],
        },
        {
          heading: 'Who is talking',
          paragraphs: [
            'With speaker identification turned on, each voice gets a label such as Person 1 or Person 2. The labels come from the sound alone; the app does not know names or accounts.',
          ],
        },
      ],
      faq: [
        { q: 'Will captions lower my FPS?', a: 'Speech recognition is light, but it shares the graphics card with the game when one is available. Translation with OPUS-MT or M2M100 runs on the processor. The Riva model is the heavy one and can slow games down.' },
        { q: 'Does it work in exclusive fullscreen?', a: 'Exclusive fullscreen can hide any overlay window. Use windowed or borderless window mode, which most games offer in the video settings.' },
        { q: 'Does it work with Discord voice chat while I play?', a: 'Yes. Anything that plays through the selected audio output is captioned, including Discord, in-game voice chat and a stream in the browser.' },
        { q: 'Can anti-cheat software flag it?', a: 'AnyCaptions does not inject code into the game or read its memory. It is a separate window that listens to the Windows audio output. Each anti-cheat has its own rules, so check the rules of the game if you are unsure.' },
      ],
    },
    'live-captions-for-calls': {
      title: 'Live captions for Discord, Meet, Zoom and Teams calls',
      description: 'Get live captions and translation for any call on Windows, in any app, without bots or plugins. AnyCaptions is free and keeps the audio on your computer.',
      answer: 'AnyCaptions shows live captions for any call on Windows 11: Discord, Google Meet, Zoom, Microsoft Teams or any other app. It captions the sound coming out of your headphones, so nothing is installed in the call and no bot joins the meeting. Captions can be translated as people speak, and the audio never leaves your computer.',
      steps: {
        heading: 'How to caption a call',
        items: [
          'Install AnyCaptions and open it.',
          'In the settings, pick the same audio output the call uses (your headphones or speakers).',
          'Join the call and click Start.',
          'To translate, choose the language you want to read in.',
        ],
      },
      sections: [
        {
          heading: 'Works with any app',
          paragraphs: [
            'The app does not connect to Discord, Meet, Zoom or Teams. It listens to the Windows audio output, so it works the same way in the desktop app, in the browser, or in a tool that has no captions of its own.',
            'The other people in the call see nothing different. The captions exist only on your screen.',
          ],
        },
        {
          heading: 'Follow a call in another language',
          paragraphs: [
            'Leave the spoken language on automatic and choose the language you want to read. English and Portuguese work right after installing. M2M100 adds 34 languages on the processor, and Riva translates into 37 on the graphics card.',
          ],
        },
        {
          heading: 'Private by design',
          paragraphs: [
            'Recognition and translation run on your computer. The audio and the text stay in memory: nothing is uploaded and nothing is saved, so there is no transcript left behind after the call.',
            'Turn on speaker identification to see labels such as Person 1 and Person 2 for each voice.',
          ],
        },
      ],
      faq: [
        { q: 'Do the other participants know I am using captions?', a: 'No. AnyCaptions runs only on your computer and does not join the call. Nobody else sees or receives anything.' },
        { q: 'Does it save a transcript of the meeting?', a: 'No. The captions are shown on the screen and then discarded. The app keeps no history.' },
        { q: 'Does it caption what I say too?', a: 'Live captions follow the audio output, which is what the other people say. For your own voice, use voice dictation, which types what you speak into any app.' },
        { q: 'Does it work with a headset?', a: 'Yes. Pick the headset as the audio output in the settings, the same device the call is using.' },
      ],
    },
    'translate-videos-in-real-time': {
      title: 'Translate videos and streams in real time',
      description: 'Read translated captions for any video, anime, course or livestream on Windows while it plays. AnyCaptions is free, works offline and needs no subtitle file.',
      answer: 'AnyCaptions translates any video while it plays on Windows 11. It listens to the audio of YouTube, a livestream, an online course or a local file, and shows translated captions in a floating window. No subtitle file and no browser extension are needed. Translation runs on your own computer, for free, in up to 37 languages.',
      steps: {
        heading: 'How to translate a video while you watch',
        items: [
          'Install AnyCaptions and open it.',
          'In the settings, pick the audio output you listen with.',
          'Choose the language you want to read the captions in.',
          'Play the video and click Start.',
        ],
      },
      sections: [
        {
          heading: 'Any player, any site',
          paragraphs: [
            'Because the app captions the sound of the computer, it works in every browser and every player: YouTube, Twitch, streaming services, video courses, recorded meetings and files on disk.',
            'It is useful when a video has no captions at all, or only captions in a language you do not read.',
          ],
        },
        {
          heading: 'Choose a translation model',
          paragraphs: [
            'OPUS-MT translates between English and Portuguese and comes with the installer. M2M100 (491 MB) translates between 34 languages on the processor. Riva (2.8 GB) translates into 37 languages and needs a graphics card with about 3 GB of free video memory.',
            'The extra models are downloaded inside the app with one click, and can be removed later to free disk space.',
          ],
        },
        {
          heading: 'Make the captions easy to read',
          paragraphs: [
            'Change the text size, colors, font, background and the width of the caption. Drag it to any place on the screen.',
          ],
        },
      ],
      faq: [
        { q: 'Does it work with Japanese audio, such as anime?', a: 'Yes. Japanese is one of the languages of the M2M100 and Riva models. Machine translation of fast or informal speech is not perfect, so expect the general meaning and not the quality of a human translator.' },
        { q: 'Do I need internet to translate?', a: 'No. After the model is on your computer, translation works offline. You only need a connection for the video itself, if it is online.' },
        { q: 'Does it work with livestreams?', a: 'Yes. Captions appear a moment after the speech, live, with no need to wait for the stream to end.' },
        { q: 'Can I save the captions to a file?', a: 'No. AnyCaptions shows the captions on the screen and keeps no history.' },
      ],
    },
    'offline-voice-dictation': {
      title: 'Offline voice dictation for Windows',
      description: 'Type with your voice in any Windows app, with no internet. AnyCaptions dictation is free, runs on your computer and can translate what you say.',
      answer: 'AnyCaptions includes voice dictation that works offline on Windows 11. Press the shortcut, speak, and the text appears where the cursor is, in any app: a chat, an email, a document or a code editor. Your voice is recognized on your own computer and is never uploaded. It can also translate what you say before typing it.',
      steps: {
        heading: 'How to dictate',
        items: [
          'Install AnyCaptions and open the Dictation tab in the settings.',
          'Choose your microphone and the language you speak.',
          'Click where you want the text and press Ctrl + Space.',
          'Speak. When you finish, the text is typed in place.',
        ],
      },
      sections: [
        {
          heading: 'Works in every app',
          paragraphs: [
            'The recognized text goes to the app that has the cursor, so there is nothing to set up for each program. The shortcut can be changed, and you choose between holding it while you speak or pressing once to start and once to stop.',
          ],
        },
        {
          heading: 'Speak one language, type another',
          paragraphs: [
            'Turn on translation in the Dictation tab and pick a target language. You speak in your language and the translated text is typed. Dictation uses the same translation model selected for the captions.',
          ],
        },
        {
          heading: 'Your voice stays on your computer',
          paragraphs: [
            'The recognition runs locally, with no account and no cloud service involved. The recording is used to produce the text and then discarded.',
          ],
        },
      ],
      faq: [
        { q: 'Is the dictation really offline?', a: 'Yes. The speech recognition model is installed with the app and runs on your computer. You can dictate with the network disconnected.' },
        { q: 'Which shortcut starts dictation?', a: 'Ctrl + Space by default. You can change it in the Dictation tab of the settings.' },
        { q: 'Does it add punctuation?', a: 'Yes. The speech recognition model writes punctuation and capital letters on its own.' },
        { q: 'Can I see the text before it is typed?', a: 'Yes. There is an option to show the recognized text on the screen while you record.' },
      ],
    },
    'windows-live-captions-alternative': {
      title: 'Alternative to Windows live captions with translation',
      description: 'AnyCaptions compared with the live captions built into Windows 11: translation on any PC, up to 37 languages, voice dictation and speaker labels. Free and offline.',
      answer: 'AnyCaptions is a free alternative to the live captions built into Windows 11. Both caption any audio on the computer without sending it to the cloud. The difference is translation: Windows translates only on Copilot+ PCs, and only into English and Simplified Chinese, while AnyCaptions translates on any Windows 11 x64 computer, into up to 37 languages.',
      steps: {
        heading: 'How to try it',
        items: [
          'Install AnyCaptions and open it.',
          'In the settings, pick the audio output you listen with.',
          'Choose the language you want to read the captions in.',
          'Play anything with speech and click Start.',
        ],
      },
      sections: [
        {
          heading: 'When Windows live captions are enough',
          paragraphs: [
            'Windows live captions come with the system and open with Win + Ctrl + L. If you only need captions in the same language that is being spoken, they do the job and there is nothing to install.',
          ],
        },
        {
          heading: 'When AnyCaptions is the better choice',
          paragraphs: [
            'Choose AnyCaptions if you want translated captions and your computer is not a Copilot+ PC, or if you want to read in a language other than English or Simplified Chinese, such as Portuguese, Spanish, French, German, Japanese or Russian.',
            'It also brings things the system captions do not have in the same place: voice dictation with optional translation, labels for who is speaking, a caption that lets clicks pass through for games, and control over colors, font, size and opacity.',
          ],
        },
      ],
      table: {
        heading: 'Side by side',
        headers: ['', 'AnyCaptions', 'Windows live captions'],
        rows: [
          ['Price', 'Free', 'Free, part of Windows 11'],
          ['Runs on your computer', 'Yes', 'Yes'],
          ['Captions any audio', 'Yes', 'Yes'],
          ['Translation', 'Any Windows 11 x64 PC', 'Copilot+ PCs only (Windows 11 24H2 or later)'],
          ['Languages you can read in', 'Up to 37', 'English and Simplified Chinese'],
          ['Voice dictation in the same app', 'Yes, with optional translation', 'No, it is a separate Windows feature'],
          ['Code', 'Public on GitHub', 'Closed'],
          ['Installation', '1.1 GB installer', 'Already in Windows'],
        ],
        source: { before: 'Windows information from ', link: "Microsoft's support page", after: ', checked in November 2026.' },
      },
      faq: [
        { q: 'Do Windows live captions translate?', a: 'According to Microsoft, live translation is available only on Copilot+ PCs running Windows 11 24H2 or later, and the translated captions come out in English or Simplified Chinese.' },
        { q: 'Can I use both?', a: 'Yes. They are independent. You can keep Windows live captions for everyday use and open AnyCaptions when you need a translation.' },
        { q: 'Does AnyCaptions need a Copilot+ PC?', a: 'No. It runs on any Windows 11 x64 computer. A graphics card makes recognition faster, and only the Riva translation model requires one.' },
        { q: 'Is AnyCaptions made by Microsoft?', a: 'No. AnyCaptions is an independent project with its code public on GitHub. It is not affiliated with Microsoft.' },
      ],
    },
  },
};
