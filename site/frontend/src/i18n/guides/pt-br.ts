import type { GuideMessages } from './types';

export const ptBr: GuideMessages = {
  label: 'Guias',
  index: {
    title: 'Guias',
    description: 'Como ter legendas ao vivo, tradução em tempo real e ditado por voz offline no Windows com o AnyCaptions.',
    heading: 'Guias',
    intro: 'Respostas curtas para o que as pessoas mais perguntam: legendas em jogos e chamadas, tradução de vídeos enquanto eles tocam e digitação com a voz.',
  },
  faqHeading: 'Perguntas frequentes',
  download: 'Baixar para Windows',
  docs: 'Ler o guia do usuário',
  more: 'Mais guias',
  homeFaq: {
    heading: 'Perguntas frequentes',
    sub: 'O que as pessoas perguntam antes de instalar.',
    items: [
      { q: 'O que é o AnyCaptions?', a: 'O AnyCaptions é um aplicativo gratuito para Windows 11 que mostra legendas ao vivo para qualquer áudio que toca no computador: jogos, vídeos, chamadas e transmissões. Ele pode traduzir as legendas enquanto você ouve e digitar o que você fala em qualquer aplicativo. O reconhecimento de fala e a tradução rodam no seu próprio computador.' },
      { q: 'O AnyCaptions é gratuito?', a: 'Sim. O aplicativo é gratuito, sem plano pago e sem anúncios, e o código é público no GitHub. Você só entra com Google, GitHub ou Discord no site para baixar o instalador.' },
      { q: 'Funciona offline?', a: 'Sim. Depois de instalado, as legendas, a tradução e o ditado funcionam sem conexão com a internet. O aplicativo só usa a rede quando você pede para baixar um modelo extra ou para verificar atualizações.' },
      { q: 'Meu áudio é enviado para algum servidor?', a: 'Não. O áudio e as legendas ficam na memória do seu computador. Nada é enviado, não há histórico e não há telemetria.' },
      { q: 'Para quais idiomas ele traduz?', a: 'Inglês e português funcionam logo após a instalação. O modelo M2M100 (491 MB, roda no processador) traduz entre 34 idiomas, e o modelo Riva (2,8 GB, precisa de placa de vídeo) traduz para 37. Os dois são baixados dentro do aplicativo.' },
      { q: 'Preciso de placa de vídeo?', a: 'Não. O reconhecimento de fala usa a placa de vídeo quando existe uma e passa para o processador quando não existe. A tradução com OPUS-MT e M2M100 roda no processador. Só o modelo Riva precisa de placa de vídeo, com cerca de 3 GB de memória de vídeo livre.' },
      { q: 'Funciona por cima de jogos?', a: 'Sim, no modo janela ou janela sem bordas. A legenda é uma janela transparente que fica por cima das outras e, depois de travada, os cliques passam por ela. A tela cheia exclusiva pode escondê-la.' },
      { q: 'Qual é a diferença para as legendas ao vivo do Windows?', a: 'As legendas ao vivo do Windows traduzem apenas em Copilot+ PCs, e apenas para inglês e chinês simplificado. O AnyCaptions traduz em qualquer computador com Windows 11 x64, para até 37 idiomas, e ainda inclui ditado por voz e identificação opcional de quem está falando.' },
    ],
  },
  guides: {
    'live-captions-for-games': {
      title: 'Legendas ao vivo para jogos no Windows',
      description: 'Mostre legendas ao vivo e traduza o chat de voz e as cutscenes de qualquer jogo de PC com o AnyCaptions. Gratuito, offline, com uma sobreposição que não bloqueia os cliques.',
      answer: 'O AnyCaptions adiciona legendas ao vivo a qualquer jogo de PC no Windows 11. Ele escuta o som que já vai para os seus fones, então o chat de voz, as cutscenes e as transmissões viram texto em uma sobreposição transparente por cima do jogo. Ele pode traduzir as legendas conforme elas aparecem. É gratuito e roda no seu próprio computador.',
      steps: {
        heading: 'Como ativar as legendas em um jogo',
        items: [
          'Instale o AnyCaptions e abra o aplicativo.',
          'Nas configurações, escolha a saída de áudio que você usa para jogar (seus fones ou caixas de som).',
          'Coloque o jogo em modo janela ou janela sem bordas.',
          'Clique em Iniciar, arraste a legenda para onde quiser e pressione Ctrl + Alt + L para travá-la.',
        ],
      },
      sections: [
        {
          heading: 'Não atrapalha o jogo',
          paragraphs: [
            'Quando a legenda está travada, os controles somem e os cliques passam direto por ela, então o mouse continua funcionando no jogo. Pressione Ctrl + Alt + L de novo para mover ou redimensionar a legenda.',
            'O AnyCaptions não lê nem altera o jogo. Ele só escuta a saída de áudio do Windows, o mesmo som que você ouve.',
          ],
        },
        {
          heading: 'Traduza colegas de equipe e cutscenes',
          paragraphs: [
            'Escolha um idioma de tradução e as legendas já saem traduzidas. Inglês e português funcionam logo após a instalação. Para outros idiomas, baixe o M2M100 (491 MB, 34 idiomas, roda no processador) dentro do aplicativo.',
            'O modelo Riva traduz para 37 idiomas, mas usa cerca de 3 GB de memória de vídeo e pode reduzir a taxa de quadros. Para jogos, o M2M100 ou o OPUS-MT são a escolha mais segura, porque não usam a placa de vídeo.',
          ],
        },
        {
          heading: 'Quem está falando',
          paragraphs: [
            'Com a identificação de falantes ativada, cada voz recebe um rótulo como Pessoa 1 ou Pessoa 2. Os rótulos vêm apenas do som; o aplicativo não conhece nomes nem contas.',
          ],
        },
      ],
      faq: [
        { q: 'As legendas vão reduzir meu FPS?', a: 'O reconhecimento de fala é leve, mas divide a placa de vídeo com o jogo quando há uma disponível. A tradução com OPUS-MT ou M2M100 roda no processador. O modelo Riva é o pesado e pode deixar os jogos mais lentos.' },
        { q: 'Funciona em tela cheia exclusiva?', a: 'A tela cheia exclusiva pode esconder qualquer janela de sobreposição. Use o modo janela ou janela sem bordas, que a maioria dos jogos oferece nas configurações de vídeo.' },
        { q: 'Funciona com o chat de voz do Discord enquanto eu jogo?', a: 'Sim. Tudo o que toca pela saída de áudio selecionada é legendado, incluindo o Discord, o chat de voz do jogo e uma transmissão no navegador.' },
        { q: 'Um anti-cheat pode detectar o aplicativo como trapaça?', a: 'O AnyCaptions não injeta código no jogo nem lê a memória dele. É uma janela separada que escuta a saída de áudio do Windows. Cada anti-cheat tem suas próprias regras, então consulte as regras do jogo se tiver dúvida.' },
      ],
    },
    'live-captions-for-calls': {
      title: 'Legendas ao vivo para chamadas no Discord, Meet, Zoom e Teams',
      description: 'Tenha legendas ao vivo e tradução em qualquer chamada no Windows, em qualquer aplicativo, sem bots nem plugins. O AnyCaptions é gratuito e mantém o áudio no seu computador.',
      answer: 'O AnyCaptions mostra legendas ao vivo para qualquer chamada no Windows 11: Discord, Google Meet, Zoom, Microsoft Teams ou qualquer outro aplicativo. Ele legenda o som que sai dos seus fones, então nada é instalado na chamada e nenhum bot entra na reunião. As legendas podem ser traduzidas enquanto as pessoas falam, e o áudio nunca sai do seu computador.',
      steps: {
        heading: 'Como legendar uma chamada',
        items: [
          'Instale o AnyCaptions e abra o aplicativo.',
          'Nas configurações, escolha a mesma saída de áudio que a chamada usa (seus fones ou caixas de som).',
          'Entre na chamada e clique em Iniciar.',
          'Para traduzir, escolha o idioma em que você quer ler.',
        ],
      },
      sections: [
        {
          heading: 'Funciona com qualquer aplicativo',
          paragraphs: [
            'O aplicativo não se conecta ao Discord, Meet, Zoom ou Teams. Ele escuta a saída de áudio do Windows, então funciona do mesmo jeito no aplicativo para desktop, no navegador ou em uma ferramenta que não tem legendas próprias.',
            'As outras pessoas da chamada não veem nada de diferente. As legendas existem só na sua tela.',
          ],
        },
        {
          heading: 'Acompanhe uma chamada em outro idioma',
          paragraphs: [
            'Deixe o idioma falado no automático e escolha o idioma em que você quer ler. Inglês e português funcionam logo após a instalação. O M2M100 adiciona 34 idiomas no processador, e o Riva traduz para 37 na placa de vídeo.',
          ],
        },
        {
          heading: 'Privado por padrão',
          paragraphs: [
            'O reconhecimento e a tradução rodam no seu computador. O áudio e o texto ficam na memória: nada é enviado e nada é salvo, então não sobra nenhuma transcrição depois da chamada.',
            'Ative a identificação de falantes para ver rótulos como Pessoa 1 e Pessoa 2 para cada voz.',
          ],
        },
      ],
      faq: [
        { q: 'Os outros participantes sabem que estou usando legendas?', a: 'Não. O AnyCaptions roda só no seu computador e não entra na chamada. Ninguém mais vê ou recebe nada.' },
        { q: 'Ele salva uma transcrição da reunião?', a: 'Não. As legendas são mostradas na tela e depois descartadas. O aplicativo não guarda histórico.' },
        { q: 'Ele também legenda o que eu falo?', a: 'As legendas ao vivo seguem a saída de áudio, que é o que as outras pessoas falam. Para a sua própria voz, use o ditado por voz, que digita o que você fala em qualquer aplicativo.' },
        { q: 'Funciona com headset?', a: 'Sim. Escolha o headset como saída de áudio nas configurações, o mesmo dispositivo que a chamada está usando.' },
      ],
    },
    'translate-videos-in-real-time': {
      title: 'Traduzir vídeos e lives em tempo real',
      description: 'Leia legendas traduzidas de qualquer vídeo, anime, curso ou transmissão ao vivo no Windows enquanto ele toca. O AnyCaptions é gratuito, funciona offline e não precisa de arquivo de legenda.',
      answer: 'O AnyCaptions traduz qualquer vídeo enquanto ele toca no Windows 11. Ele escuta o áudio do YouTube, de uma transmissão ao vivo, de um curso online ou de um arquivo local e mostra legendas traduzidas em uma janela flutuante. Não é preciso arquivo de legenda nem extensão de navegador. A tradução roda no seu próprio computador, de graça, em até 37 idiomas.',
      steps: {
        heading: 'Como traduzir um vídeo enquanto você assiste',
        items: [
          'Instale o AnyCaptions e abra o aplicativo.',
          'Nas configurações, escolha a saída de áudio que você usa para ouvir.',
          'Escolha o idioma em que você quer ler as legendas.',
          'Dê play no vídeo e clique em Iniciar.',
        ],
      },
      sections: [
        {
          heading: 'Qualquer player, qualquer site',
          paragraphs: [
            'Como o aplicativo legenda o som do computador, ele funciona em todos os navegadores e em todos os players: YouTube, Twitch, serviços de streaming, cursos em vídeo, reuniões gravadas e arquivos no disco.',
            'É útil quando um vídeo não tem legenda nenhuma, ou só tem legendas em um idioma que você não lê.',
          ],
        },
        {
          heading: 'Escolha um modelo de tradução',
          paragraphs: [
            'O OPUS-MT traduz entre inglês e português e já vem com o instalador. O M2M100 (491 MB) traduz entre 34 idiomas no processador. O Riva (2,8 GB) traduz para 37 idiomas e precisa de uma placa de vídeo com cerca de 3 GB de memória de vídeo livre.',
            'Os modelos extras são baixados dentro do aplicativo com um clique e podem ser removidos depois para liberar espaço em disco.',
          ],
        },
        {
          heading: 'Deixe as legendas fáceis de ler',
          paragraphs: [
            'Mude o tamanho do texto, as cores, a fonte, o fundo e a largura da legenda. Arraste a legenda para qualquer lugar da tela.',
          ],
        },
      ],
      faq: [
        { q: 'Funciona com áudio em japonês, como em animes?', a: 'Sim. O japonês é um dos idiomas dos modelos M2M100 e Riva. A tradução automática de fala rápida ou informal não é perfeita, então espere o sentido geral e não a qualidade de um tradutor humano.' },
        { q: 'Preciso de internet para traduzir?', a: 'Não. Depois que o modelo está no seu computador, a tradução funciona offline. Você só precisa de conexão para o vídeo em si, se ele for online.' },
        { q: 'Funciona com transmissões ao vivo?', a: 'Sim. As legendas aparecem um instante depois da fala, ao vivo, sem precisar esperar a transmissão terminar.' },
        { q: 'Posso salvar as legendas em um arquivo?', a: 'Não. O AnyCaptions mostra as legendas na tela e não guarda histórico.' },
      ],
    },
    'offline-voice-dictation': {
      title: 'Ditado por voz offline no Windows',
      description: 'Digite com a voz em qualquer aplicativo do Windows, sem internet. O ditado do AnyCaptions é gratuito, roda no seu computador e pode traduzir o que você fala.',
      answer: 'O AnyCaptions inclui um ditado por voz que funciona offline no Windows 11. Pressione o atalho, fale, e o texto aparece onde o cursor está, em qualquer aplicativo: um chat, um e-mail, um documento ou um editor de código. Sua voz é reconhecida no seu próprio computador e nunca é enviada. Ele também pode traduzir o que você fala antes de digitar.',
      steps: {
        heading: 'Como ditar',
        items: [
          'Instale o AnyCaptions e abra a aba "Ditado" nas configurações.',
          'Escolha o seu microfone e o idioma que você fala.',
          'Clique onde você quer o texto e pressione Ctrl + Espaço.',
          'Fale. Quando você terminar, o texto é digitado no lugar.',
        ],
      },
      sections: [
        {
          heading: 'Funciona em todos os aplicativos',
          paragraphs: [
            'O texto reconhecido vai para o aplicativo em que o cursor está, então não há nada para configurar em cada programa. O atalho pode ser alterado, e você escolhe entre segurá-lo enquanto fala ou pressionar uma vez para começar e outra para parar.',
          ],
        },
        {
          heading: 'Fale em um idioma, digite em outro',
          paragraphs: [
            'Ative a tradução na aba "Ditado" e escolha um idioma de destino. Você fala no seu idioma e o texto traduzido é digitado. O ditado usa o mesmo modelo de tradução selecionado para as legendas.',
          ],
        },
        {
          heading: 'Sua voz fica no seu computador',
          paragraphs: [
            'O reconhecimento roda localmente, sem conta e sem nenhum serviço de nuvem envolvido. A gravação é usada para gerar o texto e depois descartada.',
          ],
        },
      ],
      faq: [
        { q: 'O ditado é offline mesmo?', a: 'Sim. O modelo de reconhecimento de fala é instalado junto com o aplicativo e roda no seu computador. Você pode ditar com a rede desconectada.' },
        { q: 'Qual atalho inicia o ditado?', a: 'Ctrl + Espaço por padrão. Você pode alterá-lo na aba "Ditado" das configurações.' },
        { q: 'Ele coloca pontuação?', a: 'Sim. O modelo de reconhecimento de fala escreve a pontuação e as letras maiúsculas sozinho.' },
        { q: 'Posso ver o texto antes de ele ser digitado?', a: 'Sim. Há uma opção para mostrar o texto reconhecido na tela enquanto você grava.' },
      ],
    },
    'windows-live-captions-alternative': {
      title: 'Alternativa às legendas ao vivo do Windows com tradução',
      description: 'O AnyCaptions comparado com as legendas ao vivo nativas do Windows 11: tradução em qualquer PC, até 37 idiomas, ditado por voz e identificação de quem fala. Gratuito e offline.',
      answer: 'O AnyCaptions é uma alternativa gratuita às legendas ao vivo nativas do Windows 11. Os dois legendam qualquer áudio do computador sem enviá-lo para a nuvem. A diferença é a tradução: o Windows traduz apenas em Copilot+ PCs, e apenas para inglês e chinês simplificado, enquanto o AnyCaptions traduz em qualquer computador com Windows 11 x64, para até 37 idiomas.',
      steps: {
        heading: 'Como experimentar',
        items: [
          'Instale o AnyCaptions e abra o aplicativo.',
          'Nas configurações, escolha a saída de áudio que você usa para ouvir.',
          'Escolha o idioma em que você quer ler as legendas.',
          'Reproduza qualquer coisa com fala e clique em Iniciar.',
        ],
      },
      sections: [
        {
          heading: 'Quando as legendas ao vivo do Windows bastam',
          paragraphs: [
            'As legendas ao vivo do Windows vêm com o sistema e abrem com Win + Ctrl + L. Se você só precisa de legendas no mesmo idioma que está sendo falado, elas resolvem e não há nada para instalar.',
          ],
        },
        {
          heading: 'Quando o AnyCaptions é a melhor escolha',
          paragraphs: [
            'Escolha o AnyCaptions se você quer legendas traduzidas e o seu computador não é um Copilot+ PC, ou se quer ler em um idioma que não seja inglês ou chinês simplificado, como português, espanhol, francês, alemão, japonês ou russo.',
            'Ele também traz coisas que as legendas do sistema não têm no mesmo lugar: ditado por voz com tradução opcional, rótulos de quem está falando, uma legenda que deixa os cliques passarem para jogos e controle de cores, fonte, tamanho e opacidade.',
          ],
        },
      ],
      table: {
        heading: 'Lado a lado',
        headers: ['', 'AnyCaptions', 'Legendas ao vivo do Windows'],
        rows: [
          ['Preço', 'Gratuito', 'Gratuito, parte do Windows 11'],
          ['Roda no seu computador', 'Sim', 'Sim'],
          ['Legenda qualquer áudio', 'Sim', 'Sim'],
          ['Tradução', 'Qualquer PC com Windows 11 x64', 'Apenas Copilot+ PCs (Windows 11 24H2 ou posterior)'],
          ['Idiomas em que você pode ler', 'Até 37', 'Inglês e chinês simplificado'],
          ['Ditado por voz no mesmo aplicativo', 'Sim, com tradução opcional', 'Não, é um recurso separado do Windows'],
          ['Código', 'Público no GitHub', 'Fechado'],
          ['Instalação', 'Instalador de 1,1 GB', 'Já vem no Windows'],
        ],
        source: { before: 'Informações sobre o Windows retiradas da ', link: 'página de suporte da Microsoft', after: ', verificadas em novembro de 2026.' },
      },
      faq: [
        { q: 'As legendas ao vivo do Windows traduzem?', a: 'Segundo a Microsoft, a tradução ao vivo está disponível apenas em Copilot+ PCs com Windows 11 24H2 ou posterior, e as legendas traduzidas saem em inglês ou chinês simplificado.' },
        { q: 'Posso usar os dois?', a: 'Sim. Eles são independentes. Você pode manter as legendas ao vivo do Windows para o dia a dia e abrir o AnyCaptions quando precisar de tradução.' },
        { q: 'O AnyCaptions precisa de um Copilot+ PC?', a: 'Não. Ele roda em qualquer computador com Windows 11 x64. Uma placa de vídeo deixa o reconhecimento mais rápido, e só o modelo de tradução Riva exige uma.' },
        { q: 'O AnyCaptions é feito pela Microsoft?', a: 'Não. O AnyCaptions é um projeto independente com o código público no GitHub. Ele não é afiliado à Microsoft.' },
      ],
    },
  },
};
