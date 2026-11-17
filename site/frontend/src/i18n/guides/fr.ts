import type { GuideMessages } from './types';

export const fr: GuideMessages = {
  label: "Guides",
  index: {
    title: "Guides",
    description: "Comment obtenir des sous-titres en direct, la traduction en temps réel et la dictée vocale hors ligne sur Windows avec AnyCaptions.",
    heading: "Guides",
    intro: "Des réponses courtes aux questions les plus fréquentes : les sous-titres dans les jeux et les appels, la traduction de vidéos pendant la lecture et la saisie à la voix.",
  },
  faqHeading: "Questions fréquentes",
  download: "Télécharger pour Windows",
  docs: "Lire le guide d'utilisation",
  more: "Autres guides",
  homeFaq: {
    heading: "Questions fréquentes",
    sub: "Ce que l'on demande avant d'installer.",
    items: [
      { q: "Qu'est-ce qu'AnyCaptions ?", a: "AnyCaptions est une application gratuite pour Windows 11 qui affiche des sous-titres en direct pour tout son lu sur l'ordinateur : jeux, vidéos, appels et streams. Elle peut traduire les sous-titres pendant que vous écoutez et saisir ce que vous dites dans n'importe quelle application. La reconnaissance vocale et la traduction s'exécutent sur votre propre ordinateur." },
      { q: "AnyCaptions est-il gratuit ?", a: "Oui. L'application est gratuite, sans offre payante ni publicité, et son code est public sur GitHub. Vous vous connectez seulement avec Google, GitHub ou Discord sur le site pour télécharger l'installateur." },
      { q: "Est-ce que ça fonctionne hors ligne ?", a: "Oui. Après l'installation, les sous-titres, la traduction et la dictée fonctionnent sans connexion Internet. L'application n'utilise le réseau que lorsque vous lui demandez de télécharger un modèle supplémentaire ou de vérifier les mises à jour." },
      { q: "Mon audio est-il envoyé à un serveur ?", a: "Non. L'audio et les sous-titres restent dans la mémoire de votre ordinateur. Rien n'est envoyé, il n'y a ni historique ni télémétrie." },
      { q: "Quelles langues peut-il traduire ?", a: "L'anglais et le portugais fonctionnent dès l'installation. Le modèle M2M100 (491 Mo, s'exécute sur le processeur) traduit entre 34 langues, et le modèle Riva (2,8 Go, nécessite une carte graphique) traduit vers 37 langues. Les deux se téléchargent dans l'application." },
      { q: "Ai-je besoin d'une carte graphique ?", a: "Non. La reconnaissance vocale utilise la carte graphique quand il y en a une et se rabat sur le processeur quand il n'y en a pas. La traduction avec OPUS-MT et M2M100 s'exécute sur le processeur. Seul le modèle Riva nécessite une carte graphique, avec environ 3 Go de mémoire vidéo libre." },
      { q: "Est-ce que ça fonctionne par-dessus les jeux ?", a: "Oui, en mode fenêtré ou fenêtré sans bordure. Le sous-titre est une fenêtre transparente qui reste au premier plan et, une fois verrouillé, les clics passent au travers. Le plein écran exclusif peut le masquer." },
      { q: "Quelle différence avec les sous-titres en direct de Windows ?", a: "Les sous-titres en direct de Windows ne traduisent que sur les Copilot+ PC, et seulement vers l'anglais et le chinois simplifié. AnyCaptions traduit sur n'importe quel ordinateur Windows 11 x64, vers jusqu'à 37 langues, et inclut aussi la dictée vocale et des étiquettes de locuteur en option." },
    ],
  },
  guides: {
    'live-captions-for-games': {
      title: "Sous-titres en direct pour les jeux sur Windows",
      description: "Affichez des sous-titres en direct et traduisez le chat vocal et les cinématiques dans n'importe quel jeu PC avec AnyCaptions. Gratuit, hors ligne, avec une superposition qui ne bloque pas les clics.",
      answer: "AnyCaptions ajoute des sous-titres en direct à n'importe quel jeu PC sur Windows 11. Il écoute le son qui va déjà vers votre casque : le chat vocal, les cinématiques et les streams deviennent du texte dans une superposition transparente par-dessus le jeu. Il peut traduire les sous-titres à mesure qu'ils apparaissent. Il est gratuit et s'exécute sur votre propre ordinateur.",
      steps: {
        heading: "Comment activer les sous-titres dans un jeu",
        items: [
          "Installez AnyCaptions et ouvrez-le.",
          "Dans les paramètres, choisissez la sortie audio avec laquelle vous jouez (votre casque ou vos haut-parleurs).",
          "Mettez le jeu en mode fenêtré ou fenêtré sans bordure.",
          "Cliquez sur Démarrer, faites glisser le sous-titre où vous le voulez et appuyez sur Ctrl + Alt + L pour le verrouiller.",
        ],
      },
      sections: [
        {
          heading: "Il ne gêne pas le jeu",
          paragraphs: [
            "Quand le sous-titre est verrouillé, les commandes disparaissent et les clics passent directement au travers : la souris continue de fonctionner dans le jeu. Appuyez de nouveau sur Ctrl + Alt + L pour le déplacer ou le redimensionner.",
            "AnyCaptions ne lit pas le jeu et ne le modifie pas. Il écoute seulement la sortie audio de Windows, le même son que vous entendez.",
          ],
        },
        {
          heading: "Traduire les coéquipiers et les cinématiques",
          paragraphs: [
            "Choisissez une langue de traduction et les sous-titres s'affichent traduits. L'anglais et le portugais fonctionnent dès l'installation. Pour les autres langues, téléchargez M2M100 (491 Mo, 34 langues, s'exécute sur le processeur) dans l'application.",
            "Le modèle Riva traduit vers 37 langues mais utilise environ 3 Go de mémoire vidéo et peut faire baisser le nombre d'images par seconde. Pour les jeux, M2M100 ou OPUS-MT sont le choix le plus sûr, car ils ne touchent pas à la carte graphique.",
          ],
        },
        {
          heading: "Qui parle",
          paragraphs: [
            "Avec l'identification des locuteurs activée, chaque voix reçoit une étiquette comme Personne 1 ou Personne 2. Les étiquettes viennent uniquement du son ; l'application ne connaît ni les noms ni les comptes.",
          ],
        },
      ],
      faq: [
        { q: "Les sous-titres vont-ils faire baisser mes FPS ?", a: "La reconnaissance vocale est légère, mais elle partage la carte graphique avec le jeu quand il y en a une. La traduction avec OPUS-MT ou M2M100 s'exécute sur le processeur. Le modèle Riva est le plus lourd et peut ralentir les jeux." },
        { q: "Est-ce que ça fonctionne en plein écran exclusif ?", a: "Le plein écran exclusif peut masquer n'importe quelle fenêtre en superposition. Utilisez le mode fenêtré ou fenêtré sans bordure, que la plupart des jeux proposent dans les paramètres vidéo." },
        { q: "Est-ce que ça fonctionne avec le chat vocal Discord pendant que je joue ?", a: "Oui. Tout ce qui passe par la sortie audio sélectionnée est sous-titré, y compris Discord, le chat vocal du jeu et un stream dans le navigateur." },
        { q: "Un logiciel anti-triche peut-il le signaler ?", a: "AnyCaptions n'injecte pas de code dans le jeu et ne lit pas sa mémoire. C'est une fenêtre séparée qui écoute la sortie audio de Windows. Chaque anti-triche a ses propres règles : en cas de doute, consultez les règles du jeu." },
      ],
    },
    'live-captions-for-calls': {
      title: "Sous-titres en direct pour les appels Discord, Meet, Zoom et Teams",
      description: "Obtenez des sous-titres en direct et la traduction pour n'importe quel appel sur Windows, dans n'importe quelle application, sans bot ni plugin. AnyCaptions est gratuit et garde l'audio sur votre ordinateur.",
      answer: "AnyCaptions affiche des sous-titres en direct pour n'importe quel appel sur Windows 11 : Discord, Google Meet, Zoom, Microsoft Teams ou toute autre application. Il sous-titre le son qui sort de votre casque : rien n'est installé dans l'appel et aucun bot ne rejoint la réunion. Les sous-titres peuvent être traduits à mesure que les gens parlent, et l'audio ne quitte jamais votre ordinateur.",
      steps: {
        heading: "Comment sous-titrer un appel",
        items: [
          "Installez AnyCaptions et ouvrez-le.",
          "Dans les paramètres, choisissez la même sortie audio que celle de l'appel (votre casque ou vos haut-parleurs).",
          "Rejoignez l'appel et cliquez sur Démarrer.",
          "Pour traduire, choisissez la langue dans laquelle vous voulez lire.",
        ],
      },
      sections: [
        {
          heading: "Fonctionne avec n'importe quelle application",
          paragraphs: [
            "L'application ne se connecte pas à Discord, Meet, Zoom ou Teams. Elle écoute la sortie audio de Windows et fonctionne donc de la même façon dans l'application de bureau, dans le navigateur ou dans un outil qui n'a pas ses propres sous-titres.",
            "Les autres participants à l'appel ne voient rien de différent. Les sous-titres n'existent que sur votre écran.",
          ],
        },
        {
          heading: "Suivre un appel dans une autre langue",
          paragraphs: [
            "Laissez la langue parlée sur automatique et choisissez la langue dans laquelle vous voulez lire. L'anglais et le portugais fonctionnent dès l'installation. M2M100 ajoute 34 langues sur le processeur, et Riva traduit vers 37 langues sur la carte graphique.",
          ],
        },
        {
          heading: "Privé par conception",
          paragraphs: [
            "La reconnaissance et la traduction s'exécutent sur votre ordinateur. L'audio et le texte restent en mémoire : rien n'est envoyé et rien n'est enregistré, il ne reste donc aucune transcription après l'appel.",
            "Activez l'identification des locuteurs pour voir des étiquettes comme Personne 1 et Personne 2 pour chaque voix.",
          ],
        },
      ],
      faq: [
        { q: "Les autres participants savent-ils que j'utilise des sous-titres ?", a: "Non. AnyCaptions s'exécute uniquement sur votre ordinateur et ne rejoint pas l'appel. Personne d'autre ne voit ni ne reçoit quoi que ce soit." },
        { q: "Enregistre-t-il une transcription de la réunion ?", a: "Non. Les sous-titres sont affichés à l'écran puis supprimés. L'application ne garde aucun historique." },
        { q: "Sous-titre-t-il aussi ce que je dis ?", a: "Les sous-titres en direct suivent la sortie audio, c'est-à-dire ce que disent les autres. Pour votre propre voix, utilisez la dictée vocale, qui saisit ce que vous dites dans n'importe quelle application." },
        { q: "Est-ce que ça fonctionne avec un micro-casque ?", a: "Oui. Choisissez le micro-casque comme sortie audio dans les paramètres, le même appareil que celui utilisé par l'appel." },
      ],
    },
    'translate-videos-in-real-time': {
      title: "Traduire des vidéos et des streams en temps réel",
      description: "Lisez des sous-titres traduits pour n'importe quelle vidéo, anime, cours ou diffusion en direct sur Windows pendant la lecture. AnyCaptions est gratuit, fonctionne hors ligne et n'a besoin d'aucun fichier de sous-titres.",
      answer: "AnyCaptions traduit n'importe quelle vidéo pendant sa lecture sur Windows 11. Il écoute l'audio de YouTube, d'une diffusion en direct, d'un cours en ligne ou d'un fichier local, et affiche des sous-titres traduits dans une fenêtre flottante. Aucun fichier de sous-titres ni aucune extension de navigateur ne sont nécessaires. La traduction s'exécute sur votre propre ordinateur, gratuitement, dans jusqu'à 37 langues.",
      steps: {
        heading: "Comment traduire une vidéo pendant que vous la regardez",
        items: [
          "Installez AnyCaptions et ouvrez-le.",
          "Dans les paramètres, choisissez la sortie audio avec laquelle vous écoutez.",
          "Choisissez la langue dans laquelle vous voulez lire les sous-titres.",
          "Lancez la vidéo et cliquez sur Démarrer.",
        ],
      },
      sections: [
        {
          heading: "N'importe quel lecteur, n'importe quel site",
          paragraphs: [
            "Comme l'application sous-titre le son de l'ordinateur, elle fonctionne dans tous les navigateurs et tous les lecteurs : YouTube, Twitch, services de streaming, cours vidéo, réunions enregistrées et fichiers sur le disque.",
            "C'est utile quand une vidéo n'a aucun sous-titre, ou seulement des sous-titres dans une langue que vous ne lisez pas.",
          ],
        },
        {
          heading: "Choisir un modèle de traduction",
          paragraphs: [
            "OPUS-MT traduit entre l'anglais et le portugais et est fourni avec l'installateur. M2M100 (491 Mo) traduit entre 34 langues sur le processeur. Riva (2,8 Go) traduit vers 37 langues et nécessite une carte graphique avec environ 3 Go de mémoire vidéo libre.",
            "Les modèles supplémentaires se téléchargent dans l'application en un clic et peuvent être supprimés plus tard pour libérer de l'espace disque.",
          ],
        },
        {
          heading: "Rendre les sous-titres faciles à lire",
          paragraphs: [
            "Modifiez la taille du texte, les couleurs, la police, l'arrière-plan et la largeur du sous-titre. Faites-le glisser n'importe où sur l'écran.",
          ],
        },
      ],
      faq: [
        { q: "Est-ce que ça fonctionne avec de l'audio en japonais, comme les animes ?", a: "Oui. Le japonais fait partie des langues des modèles M2M100 et Riva. La traduction automatique d'une parole rapide ou familière n'est pas parfaite : attendez-vous au sens général et non à la qualité d'un traducteur humain." },
        { q: "Ai-je besoin d'Internet pour traduire ?", a: "Non. Une fois le modèle sur votre ordinateur, la traduction fonctionne hors ligne. Vous n'avez besoin d'une connexion que pour la vidéo elle-même, si elle est en ligne." },
        { q: "Est-ce que ça fonctionne avec les diffusions en direct ?", a: "Oui. Les sous-titres apparaissent un instant après la parole, en direct, sans attendre la fin de la diffusion." },
        { q: "Puis-je enregistrer les sous-titres dans un fichier ?", a: "Non. AnyCaptions affiche les sous-titres à l'écran et ne garde aucun historique." },
      ],
    },
    'offline-voice-dictation': {
      title: "Dictée vocale hors ligne pour Windows",
      description: "Écrivez à la voix dans n'importe quelle application Windows, sans Internet. La dictée d'AnyCaptions est gratuite, s'exécute sur votre ordinateur et peut traduire ce que vous dites.",
      answer: "AnyCaptions inclut une dictée vocale qui fonctionne hors ligne sur Windows 11. Appuyez sur le raccourci, parlez, et le texte apparaît à l'emplacement du curseur, dans n'importe quelle application : une discussion, un e-mail, un document ou un éditeur de code. Votre voix est reconnue sur votre propre ordinateur et n'est jamais envoyée. Il peut aussi traduire ce que vous dites avant de le saisir.",
      steps: {
        heading: "Comment dicter",
        items: [
          "Installez AnyCaptions et ouvrez l'onglet Dictée dans les paramètres.",
          "Choisissez votre microphone et la langue que vous parlez.",
          "Cliquez là où vous voulez le texte et appuyez sur Ctrl + Espace.",
          "Parlez. Quand vous avez terminé, le texte est saisi à cet endroit.",
        ],
      },
      sections: [
        {
          heading: "Fonctionne dans toutes les applications",
          paragraphs: [
            "Le texte reconnu va dans l'application où se trouve le curseur : il n'y a rien à configurer pour chaque programme. Le raccourci peut être modifié, et vous choisissez entre le maintenir pendant que vous parlez ou appuyer une fois pour démarrer et une fois pour arrêter.",
          ],
        },
        {
          heading: "Parler une langue, écrire dans une autre",
          paragraphs: [
            "Activez la traduction dans l'onglet Dictée et choisissez une langue cible. Vous parlez dans votre langue et le texte traduit est saisi. La dictée utilise le même modèle de traduction que celui sélectionné pour les sous-titres.",
          ],
        },
        {
          heading: "Votre voix reste sur votre ordinateur",
          paragraphs: [
            "La reconnaissance s'exécute en local, sans compte ni service cloud. L'enregistrement sert à produire le texte, puis il est supprimé.",
          ],
        },
      ],
      faq: [
        { q: "La dictée est-elle vraiment hors ligne ?", a: "Oui. Le modèle de reconnaissance vocale est installé avec l'application et s'exécute sur votre ordinateur. Vous pouvez dicter avec le réseau déconnecté." },
        { q: "Quel raccourci lance la dictée ?", a: "Ctrl + Espace par défaut. Vous pouvez le modifier dans l'onglet Dictée des paramètres." },
        { q: "Ajoute-t-elle la ponctuation ?", a: "Oui. Le modèle de reconnaissance vocale écrit de lui-même la ponctuation et les majuscules." },
        { q: "Puis-je voir le texte avant qu'il soit saisi ?", a: "Oui. Une option permet d'afficher le texte reconnu à l'écran pendant l'enregistrement." },
      ],
    },
    'windows-live-captions-alternative': {
      title: "Alternative aux sous-titres en direct de Windows avec traduction",
      description: "AnyCaptions comparé aux sous-titres en direct intégrés à Windows 11 : traduction sur n'importe quel PC, jusqu'à 37 langues, dictée vocale et étiquettes de locuteur. Gratuit et hors ligne.",
      answer: "AnyCaptions est une alternative gratuite aux sous-titres en direct intégrés à Windows 11. Les deux sous-titrent n'importe quel son de l'ordinateur sans l'envoyer dans le cloud. La différence est la traduction : Windows ne traduit que sur les Copilot+ PC, et seulement vers l'anglais et le chinois simplifié, alors qu'AnyCaptions traduit sur n'importe quel ordinateur Windows 11 x64, vers jusqu'à 37 langues.",
      steps: {
        heading: "Comment l'essayer",
        items: [
          "Installez AnyCaptions et ouvrez-le.",
          "Dans les paramètres, choisissez la sortie audio avec laquelle vous écoutez.",
          "Choisissez la langue dans laquelle vous voulez lire les sous-titres.",
          "Lancez n'importe quel contenu avec de la parole et cliquez sur Démarrer.",
        ],
      },
      sections: [
        {
          heading: "Quand les sous-titres en direct de Windows suffisent",
          paragraphs: [
            "Les sous-titres en direct de Windows sont fournis avec le système et s'ouvrent avec Win + Ctrl + L. Si vous avez seulement besoin de sous-titres dans la langue qui est parlée, ils font l'affaire et il n'y a rien à installer.",
          ],
        },
        {
          heading: "Quand AnyCaptions est le meilleur choix",
          paragraphs: [
            "Choisissez AnyCaptions si vous voulez des sous-titres traduits et que votre ordinateur n'est pas un Copilot+ PC, ou si vous voulez lire dans une langue autre que l'anglais ou le chinois simplifié, comme le portugais, l'espagnol, le français, l'allemand, le japonais ou le russe.",
            "Il apporte aussi des fonctions que les sous-titres du système n'ont pas au même endroit : la dictée vocale avec traduction en option, des étiquettes indiquant qui parle, un sous-titre qui laisse passer les clics pour les jeux, et le réglage des couleurs, de la police, de la taille et de l'opacité.",
          ],
        },
      ],
      table: {
        heading: "Côte à côte",
        headers: ["", "AnyCaptions", "Sous-titres en direct de Windows"],
        rows: [
          ["Prix", "Gratuit", "Gratuit, inclus dans Windows 11"],
          ["S'exécute sur votre ordinateur", "Oui", "Oui"],
          ["Sous-titre n'importe quel son", "Oui", "Oui"],
          ["Traduction", "N'importe quel PC Windows 11 x64", "Copilot+ PC uniquement (Windows 11 24H2 ou version ultérieure)"],
          ["Langues dans lesquelles vous pouvez lire", "Jusqu'à 37", "Anglais et chinois simplifié"],
          ["Dictée vocale dans la même application", "Oui, avec traduction en option", "Non, c'est une fonction distincte de Windows"],
          ["Code", "Public sur GitHub", "Fermé"],
          ["Installation", "Installateur de 1,1 Go", "Déjà dans Windows"],
        ],
        source: { before: "Informations sur Windows tirées de ", link: "la page d'assistance de Microsoft", after: ", vérifiées en novembre 2026." },
      },
      faq: [
        { q: "Les sous-titres en direct de Windows traduisent-ils ?", a: "Selon Microsoft, la traduction en direct est disponible uniquement sur les Copilot+ PC sous Windows 11 24H2 ou version ultérieure, et les sous-titres traduits s'affichent en anglais ou en chinois simplifié." },
        { q: "Puis-je utiliser les deux ?", a: "Oui. Ils sont indépendants. Vous pouvez garder les sous-titres en direct de Windows pour l'usage quotidien et ouvrir AnyCaptions quand vous avez besoin d'une traduction." },
        { q: "AnyCaptions nécessite-t-il un Copilot+ PC ?", a: "Non. Il s'exécute sur n'importe quel ordinateur Windows 11 x64. Une carte graphique accélère la reconnaissance, et seul le modèle de traduction Riva en nécessite une." },
        { q: "AnyCaptions est-il fait par Microsoft ?", a: "Non. AnyCaptions est un projet indépendant dont le code est public sur GitHub. Il n'est pas affilié à Microsoft." },
      ],
    },
  },
};
