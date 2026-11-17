import type { GuideMessages } from './types';

export const es: GuideMessages = {
  label: 'Guías',
  index: {
    title: 'Guías',
    description: 'Cómo tener subtítulos en vivo, traducción en tiempo real y dictado por voz sin conexión en Windows con AnyCaptions.',
    heading: 'Guías',
    intro: 'Respuestas cortas a lo que más se pregunta: subtítulos en juegos y llamadas, traducir videos mientras se reproducen y escribir con la voz.',
  },
  faqHeading: 'Preguntas frecuentes',
  download: 'Descargar para Windows',
  docs: 'Leer la guía de uso',
  more: 'Más guías',
  homeFaq: {
    heading: 'Preguntas frecuentes',
    sub: 'Lo que se pregunta antes de instalar.',
    items: [
      { q: '¿Qué es AnyCaptions?', a: 'AnyCaptions es una aplicación gratuita para Windows 11 que muestra subtítulos en vivo de cualquier audio que suene en el ordenador: juegos, videos, llamadas y transmisiones. Puede traducir los subtítulos mientras escuchas y escribir lo que dices en cualquier aplicación. El reconocimiento de voz y la traducción se ejecutan en tu propio ordenador.' },
      { q: '¿AnyCaptions es gratis?', a: 'Sí. La aplicación es gratuita, sin plan de pago y sin anuncios, y su código es público en GitHub. Solo inicias sesión con Google, GitHub o Discord en el sitio web para descargar el instalador.' },
      { q: '¿Funciona sin conexión?', a: 'Sí. Después de instalarla, los subtítulos, la traducción y el dictado funcionan sin conexión a internet. La aplicación solo usa la red cuando le pides que descargue un modelo adicional o que busque actualizaciones.' },
      { q: '¿Mi audio se envía a un servidor?', a: 'No. El audio y los subtítulos se quedan en la memoria de tu ordenador. No se sube nada, no hay historial ni telemetría.' },
      { q: '¿A qué idiomas puede traducir?', a: 'El inglés y el portugués funcionan nada más instalar. El modelo M2M100 (491 MB, se ejecuta en el procesador) traduce entre 34 idiomas, y el modelo Riva (2.8 GB, necesita una tarjeta gráfica) traduce a 37. Ambos se descargan dentro de la aplicación.' },
      { q: '¿Necesito una tarjeta gráfica?', a: 'No. El reconocimiento de voz usa la tarjeta gráfica cuando hay una y recurre al procesador cuando no la hay. La traducción con OPUS-MT y M2M100 se ejecuta en el procesador. Solo el modelo Riva necesita una tarjeta gráfica, con unos 3 GB de memoria de video libre.' },
      { q: '¿Funciona sobre los juegos?', a: 'Sí, en modo ventana o ventana sin bordes. El subtítulo es una ventana transparente que se mantiene encima y, una vez bloqueado, los clics lo atraviesan. La pantalla completa exclusiva puede ocultarlo.' },
      { q: '¿En qué se diferencia de los subtítulos en vivo de Windows?', a: 'Los subtítulos en vivo de Windows solo traducen en equipos Copilot+ PC, y solo al inglés y al chino simplificado. AnyCaptions traduce en cualquier ordenador con Windows 11 x64, hasta a 37 idiomas, y además incluye dictado por voz y etiquetas opcionales de hablantes.' },
    ],
  },
  guides: {
    'live-captions-for-games': {
      title: 'Subtítulos en vivo para juegos en Windows',
      description: 'Muestra subtítulos en vivo y traduce el chat de voz y las cinemáticas de cualquier juego de PC con AnyCaptions. Gratis, sin conexión, con una superposición que no bloquea los clics.',
      answer: 'AnyCaptions añade subtítulos en vivo a cualquier juego de PC en Windows 11. Escucha el sonido que ya llega a tus auriculares, así que el chat de voz, las cinemáticas y las transmisiones se convierten en texto en una superposición transparente sobre el juego. Puede traducir los subtítulos a medida que aparecen. Es gratis y se ejecuta en tu propio ordenador.',
      steps: {
        heading: 'Cómo activar los subtítulos en un juego',
        items: [
          'Instala AnyCaptions y ábrela.',
          'En la configuración, elige la salida de audio con la que juegas (tus auriculares o altavoces).',
          'Pon el juego en modo ventana o ventana sin bordes.',
          'Haz clic en Iniciar, arrastra el subtítulo adonde quieras y pulsa Ctrl + Alt + L para bloquearlo.',
        ],
      },
      sections: [
        {
          heading: 'No estorba en el juego',
          paragraphs: [
            'Cuando el subtítulo está bloqueado, los controles desaparecen y los clics lo atraviesan, así que el ratón sigue funcionando en el juego. Pulsa Ctrl + Alt + L otra vez para moverlo o cambiar su tamaño.',
            'AnyCaptions no lee ni modifica el juego. Solo escucha la salida de audio de Windows, el mismo sonido que oyes tú.',
          ],
        },
        {
          heading: 'Traduce a tus compañeros y las cinemáticas',
          paragraphs: [
            'Elige un idioma de traducción y los subtítulos salen traducidos. El inglés y el portugués funcionan nada más instalar. Para otros idiomas, descarga M2M100 (491 MB, 34 idiomas, se ejecuta en el procesador) dentro de la aplicación.',
            'El modelo Riva traduce a 37 idiomas, pero usa unos 3 GB de memoria de video y puede bajar los fotogramas por segundo. Para juegos, M2M100 u OPUS-MT son la opción más segura porque no tocan la tarjeta gráfica.',
          ],
        },
        {
          heading: 'Quién está hablando',
          paragraphs: [
            'Con la identificación de hablantes activada, cada voz recibe una etiqueta como Persona 1 o Persona 2. Las etiquetas salen solo del sonido; la aplicación no conoce nombres ni cuentas.',
          ],
        },
      ],
      faq: [
        { q: '¿Los subtítulos bajan mis FPS?', a: 'El reconocimiento de voz es ligero, pero comparte la tarjeta gráfica con el juego cuando hay una disponible. La traducción con OPUS-MT o M2M100 se ejecuta en el procesador. El modelo Riva es el pesado y puede ralentizar los juegos.' },
        { q: '¿Funciona en pantalla completa exclusiva?', a: 'La pantalla completa exclusiva puede ocultar cualquier ventana superpuesta. Usa el modo ventana o ventana sin bordes, que la mayoría de los juegos ofrece en los ajustes de video.' },
        { q: '¿Funciona con el chat de voz de Discord mientras juego?', a: 'Sí. Todo lo que suena por la salida de audio seleccionada recibe subtítulos, incluido Discord, el chat de voz del juego y una transmisión en el navegador.' },
        { q: '¿Puede detectarlo un programa antitrampas?', a: 'AnyCaptions no inyecta código en el juego ni lee su memoria. Es una ventana aparte que escucha la salida de audio de Windows. Cada sistema antitrampas tiene sus propias reglas, así que consulta las reglas del juego si tienes dudas.' },
      ],
    },
    'live-captions-for-calls': {
      title: 'Subtítulos en vivo para llamadas de Discord, Meet, Zoom y Teams',
      description: 'Ten subtítulos en vivo y traducción en cualquier llamada en Windows, en cualquier aplicación, sin bots ni complementos. AnyCaptions es gratis y mantiene el audio en tu ordenador.',
      answer: 'AnyCaptions muestra subtítulos en vivo de cualquier llamada en Windows 11: Discord, Google Meet, Zoom, Microsoft Teams o cualquier otra aplicación. Subtitula el sonido que sale por tus auriculares, así que no se instala nada en la llamada y ningún bot entra en la reunión. Los subtítulos se pueden traducir mientras las personas hablan, y el audio nunca sale de tu ordenador.',
      steps: {
        heading: 'Cómo subtitular una llamada',
        items: [
          'Instala AnyCaptions y ábrela.',
          'En la configuración, elige la misma salida de audio que usa la llamada (tus auriculares o altavoces).',
          'Entra en la llamada y haz clic en Iniciar.',
          'Para traducir, elige el idioma en el que quieres leer.',
        ],
      },
      sections: [
        {
          heading: 'Funciona con cualquier aplicación',
          paragraphs: [
            'La aplicación no se conecta a Discord, Meet, Zoom ni Teams. Escucha la salida de audio de Windows, así que funciona igual en la aplicación de escritorio, en el navegador o en una herramienta que no tiene subtítulos propios.',
            'Las demás personas de la llamada no ven nada distinto. Los subtítulos existen solo en tu pantalla.',
          ],
        },
        {
          heading: 'Sigue una llamada en otro idioma',
          paragraphs: [
            'Deja el idioma hablado en automático y elige el idioma en el que quieres leer. El inglés y el portugués funcionan nada más instalar. M2M100 añade 34 idiomas en el procesador, y Riva traduce a 37 en la tarjeta gráfica.',
          ],
        },
        {
          heading: 'Privada por diseño',
          paragraphs: [
            'El reconocimiento y la traducción se ejecutan en tu ordenador. El audio y el texto se quedan en la memoria: no se sube nada y no se guarda nada, así que no queda ninguna transcripción después de la llamada.',
            'Activa la identificación de hablantes para ver etiquetas como Persona 1 y Persona 2 para cada voz.',
          ],
        },
      ],
      faq: [
        { q: '¿Los demás participantes saben que uso subtítulos?', a: 'No. AnyCaptions se ejecuta solo en tu ordenador y no entra en la llamada. Nadie más ve ni recibe nada.' },
        { q: '¿Guarda una transcripción de la reunión?', a: 'No. Los subtítulos se muestran en la pantalla y luego se descartan. La aplicación no guarda ningún historial.' },
        { q: '¿También subtitula lo que digo yo?', a: 'Los subtítulos en vivo siguen la salida de audio, que es lo que dicen las otras personas. Para tu propia voz, usa el dictado por voz, que escribe lo que dices en cualquier aplicación.' },
        { q: '¿Funciona con auriculares con micrófono?', a: 'Sí. Elige los auriculares como salida de audio en la configuración, el mismo dispositivo que usa la llamada.' },
      ],
    },
    'translate-videos-in-real-time': {
      title: 'Traducir videos y transmisiones en tiempo real',
      description: 'Lee subtítulos traducidos de cualquier video, anime, curso o transmisión en vivo en Windows mientras se reproduce. AnyCaptions es gratis, funciona sin conexión y no necesita archivo de subtítulos.',
      answer: 'AnyCaptions traduce cualquier video mientras se reproduce en Windows 11. Escucha el audio de YouTube, de una transmisión en vivo, de un curso en línea o de un archivo local, y muestra subtítulos traducidos en una ventana flotante. No hace falta ningún archivo de subtítulos ni ninguna extensión del navegador. La traducción se ejecuta en tu propio ordenador, gratis, en hasta 37 idiomas.',
      steps: {
        heading: 'Cómo traducir un video mientras lo ves',
        items: [
          'Instala AnyCaptions y ábrela.',
          'En la configuración, elige la salida de audio con la que escuchas.',
          'Elige el idioma en el que quieres leer los subtítulos.',
          'Reproduce el video y haz clic en Iniciar.',
        ],
      },
      sections: [
        {
          heading: 'Cualquier reproductor, cualquier sitio',
          paragraphs: [
            'Como la aplicación subtitula el sonido del ordenador, funciona en todos los navegadores y en todos los reproductores: YouTube, Twitch, servicios de streaming, cursos en video, reuniones grabadas y archivos en el disco.',
            'Es útil cuando un video no tiene subtítulos, o solo los tiene en un idioma que no lees.',
          ],
        },
        {
          heading: 'Elige un modelo de traducción',
          paragraphs: [
            'OPUS-MT traduce entre inglés y portugués y viene con el instalador. M2M100 (491 MB) traduce entre 34 idiomas en el procesador. Riva (2.8 GB) traduce a 37 idiomas y necesita una tarjeta gráfica con unos 3 GB de memoria de video libre.',
            'Los modelos adicionales se descargan dentro de la aplicación con un clic, y se pueden eliminar después para liberar espacio en disco.',
          ],
        },
        {
          heading: 'Haz que los subtítulos sean fáciles de leer',
          paragraphs: [
            'Cambia el tamaño del texto, los colores, la fuente, el fondo y el ancho del subtítulo. Arrástralo a cualquier lugar de la pantalla.',
          ],
        },
      ],
      faq: [
        { q: '¿Funciona con audio en japonés, como el anime?', a: 'Sí. El japonés es uno de los idiomas de los modelos M2M100 y Riva. La traducción automática del habla rápida o informal no es perfecta, así que espera el sentido general y no la calidad de un traductor humano.' },
        { q: '¿Necesito internet para traducir?', a: 'No. Una vez que el modelo está en tu ordenador, la traducción funciona sin conexión. Solo necesitas conexión para el video en sí, si está en línea.' },
        { q: '¿Funciona con transmisiones en vivo?', a: 'Sí. Los subtítulos aparecen un momento después de la voz, en vivo, sin necesidad de esperar a que termine la transmisión.' },
        { q: '¿Puedo guardar los subtítulos en un archivo?', a: 'No. AnyCaptions muestra los subtítulos en la pantalla y no guarda ningún historial.' },
      ],
    },
    'offline-voice-dictation': {
      title: 'Dictado por voz sin conexión para Windows',
      description: 'Escribe con la voz en cualquier aplicación de Windows, sin internet. El dictado de AnyCaptions es gratis, se ejecuta en tu ordenador y puede traducir lo que dices.',
      answer: 'AnyCaptions incluye un dictado por voz que funciona sin conexión en Windows 11. Pulsa el atajo, habla, y el texto aparece donde está el cursor, en cualquier aplicación: un chat, un correo, un documento o un editor de código. Tu voz se reconoce en tu propio ordenador y nunca se sube. También puede traducir lo que dices antes de escribirlo.',
      steps: {
        heading: 'Cómo dictar',
        items: [
          'Instala AnyCaptions y abre la pestaña "Dictado" en la configuración.',
          'Elige tu micrófono y el idioma que hablas.',
          'Haz clic donde quieres el texto y pulsa Ctrl + Espacio.',
          'Habla. Cuando terminas, el texto se escribe en ese lugar.',
        ],
      },
      sections: [
        {
          heading: 'Funciona en todas las aplicaciones',
          paragraphs: [
            'El texto reconocido va a la aplicación que tiene el cursor, así que no hay nada que configurar para cada programa. El atajo se puede cambiar, y tú eliges entre mantenerlo pulsado mientras hablas o pulsarlo una vez para empezar y otra para parar.',
          ],
        },
        {
          heading: 'Habla en un idioma, escribe en otro',
          paragraphs: [
            'Activa la traducción en la pestaña "Dictado" y elige un idioma de destino. Hablas en tu idioma y se escribe el texto traducido. El dictado usa el mismo modelo de traducción seleccionado para los subtítulos.',
          ],
        },
        {
          heading: 'Tu voz se queda en tu ordenador',
          paragraphs: [
            'El reconocimiento se ejecuta de forma local, sin cuenta y sin ningún servicio en la nube. La grabación se usa para generar el texto y luego se descarta.',
          ],
        },
      ],
      faq: [
        { q: '¿El dictado funciona de verdad sin conexión?', a: 'Sí. El modelo de reconocimiento de voz se instala con la aplicación y se ejecuta en tu ordenador. Puedes dictar con la red desconectada.' },
        { q: '¿Qué atajo inicia el dictado?', a: 'Ctrl + Espacio de forma predeterminada. Puedes cambiarlo en la pestaña "Dictado" de la configuración.' },
        { q: '¿Añade puntuación?', a: 'Sí. El modelo de reconocimiento de voz escribe la puntuación y las mayúsculas por sí solo.' },
        { q: '¿Puedo ver el texto antes de que se escriba?', a: 'Sí. Hay una opción para mostrar el texto reconocido en la pantalla mientras grabas.' },
      ],
    },
    'windows-live-captions-alternative': {
      title: 'Alternativa a los subtítulos en vivo de Windows con traducción',
      description: 'AnyCaptions comparada con los subtítulos en vivo integrados en Windows 11: traducción en cualquier PC, hasta 37 idiomas, dictado por voz y etiquetas de hablantes. Gratis y sin conexión.',
      answer: 'AnyCaptions es una alternativa gratuita a los subtítulos en vivo integrados en Windows 11. Ambos subtitulan cualquier audio del ordenador sin enviarlo a la nube. La diferencia está en la traducción: Windows solo traduce en equipos Copilot+ PC, y solo al inglés y al chino simplificado, mientras que AnyCaptions traduce en cualquier ordenador con Windows 11 x64, hasta a 37 idiomas.',
      steps: {
        heading: 'Cómo probarla',
        items: [
          'Instala AnyCaptions y ábrela.',
          'En la configuración, elige la salida de audio con la que escuchas.',
          'Elige el idioma en el que quieres leer los subtítulos.',
          'Reproduce cualquier cosa con voz y haz clic en Iniciar.',
        ],
      },
      sections: [
        {
          heading: 'Cuándo bastan los subtítulos en vivo de Windows',
          paragraphs: [
            'Los subtítulos en vivo de Windows vienen con el sistema y se abren con Win + Ctrl + L. Si solo necesitas subtítulos en el mismo idioma que se está hablando, cumplen su función y no hay nada que instalar.',
          ],
        },
        {
          heading: 'Cuándo AnyCaptions es la mejor opción',
          paragraphs: [
            'Elige AnyCaptions si quieres subtítulos traducidos y tu ordenador no es un Copilot+ PC, o si quieres leer en un idioma que no sea inglés ni chino simplificado, como portugués, español, francés, alemán, japonés o ruso.',
            'También trae cosas que los subtítulos del sistema no tienen en el mismo lugar: dictado por voz con traducción opcional, etiquetas de quién está hablando, un subtítulo que deja pasar los clics para los juegos, y control sobre los colores, la fuente, el tamaño y la opacidad.',
          ],
        },
      ],
      table: {
        heading: 'Comparación',
        headers: ['', 'AnyCaptions', 'Subtítulos en vivo de Windows'],
        rows: [
          ['Precio', 'Gratis', 'Gratis, parte de Windows 11'],
          ['Se ejecuta en tu ordenador', 'Sí', 'Sí'],
          ['Subtitula cualquier audio', 'Sí', 'Sí'],
          ['Traducción', 'Cualquier PC con Windows 11 x64', 'Solo equipos Copilot+ PC (Windows 11 24H2 o posterior)'],
          ['Idiomas en los que puedes leer', 'Hasta 37', 'Inglés y chino simplificado'],
          ['Dictado por voz en la misma aplicación', 'Sí, con traducción opcional', 'No, es una función aparte de Windows'],
          ['Código', 'Público en GitHub', 'Cerrado'],
          ['Instalación', 'Instalador de 1.1 GB', 'Ya está en Windows'],
        ],
        source: { before: 'Información de Windows tomada de ', link: 'la página de soporte de Microsoft', after: ', consultada en noviembre de 2026.' },
      },
      faq: [
        { q: '¿Los subtítulos en vivo de Windows traducen?', a: 'Según Microsoft, la traducción en vivo solo está disponible en equipos Copilot+ PC con Windows 11 24H2 o posterior, y los subtítulos traducidos salen en inglés o chino simplificado.' },
        { q: '¿Puedo usar los dos?', a: 'Sí. Son independientes. Puedes dejar los subtítulos en vivo de Windows para el uso diario y abrir AnyCaptions cuando necesites una traducción.' },
        { q: '¿AnyCaptions necesita un Copilot+ PC?', a: 'No. Funciona en cualquier ordenador con Windows 11 x64. Una tarjeta gráfica hace más rápido el reconocimiento, y solo el modelo de traducción Riva la requiere.' },
        { q: '¿AnyCaptions es de Microsoft?', a: 'No. AnyCaptions es un proyecto independiente con su código público en GitHub. No está afiliado a Microsoft.' },
      ],
    },
  },
};
