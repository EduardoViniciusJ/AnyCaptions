import { Fragment } from "react";
import { speakerLabel, type CaptionFrame, type CaptionPart } from "../captions";
/** Rótulo genérico do falante ("Pessoa 1:"); a cor é só um reforço, o texto é a pista. */
function SpeakerLabel({ speaker }: { speaker: number }) {
  return (
    <span className="caption-speaker" data-speaker={Math.min(speaker, 8)}>
      {speakerLabel(speaker)}{" "}
    </span>
  );
}
function Part({ part }: { part: CaptionPart }) {
  return (
    <>
      {part.speaker !== undefined && <SpeakerLabel speaker={part.speaker} />}
      <span className={part.partial ? "caption-partial" : "caption-final"}>
        {part.text}
      </span>
    </>
  );
}
/** Agrupa os trechos em turnos: um começa no primeiro trecho e em cada rótulo. */
function turns(parts: readonly CaptionPart[]): CaptionPart[][] {
  const groups: CaptionPart[][] = [];
  parts.forEach((part, index) => {
    if (index === 0 || part.speaker !== undefined) groups.push([part]);
    else groups.at(-1)!.push(part);
  });
  return groups;
}
export function CaptionText({ frame }: { frame?: CaptionFrame }) {
  if (!frame?.text) return null;
  return (
    <span className="caption-content">
      {frame.parts && frame.lines === "multi" ? (
        // Rótulos ligados: um turno de fala por linha.
        turns(frame.parts).map((turn, index) => (
          <Fragment key={index}>
            {index > 0 ? " " : ""}
            <span className="caption-turn">
              {turn.map((part, partIndex) => (
                <Fragment key={partIndex}>
                  {partIndex > 0 ? " " : ""}
                  <Part part={part} />
                </Fragment>
              ))}
            </span>
          </Fragment>
        ))
      ) : frame.parts ? (
        frame.parts.map((part, index) => (
          <Fragment key={index}>
            {index > 0 ? " " : ""}
            <Part part={part} />
          </Fragment>
        ))
      ) : frame.livePartialText ? (
        <>
          {frame.committedText && (
            <span className="caption-final">{frame.committedText} </span>
          )}
          <span className="caption-partial">{frame.livePartialText}</span>
        </>
      ) : frame.sentences.length ? (
        frame.sentences.map((sentence, index) => {
          const speaker = frame.sentenceSpeakers?.[index];
          // Com rótulos, cada falante começa na própria linha.
          const breakLine = index > 0 && speaker !== undefined;
          return (
            <Fragment key={index}>
              {index > 0 ? " " : ""}
              {breakLine && <span className="caption-turn-break" />}
              {speaker !== undefined && <SpeakerLabel speaker={speaker} />}
              <span
                className={`caption-sentence${frame.animateNewest && index === frame.sentences.length - 1 ? " caption-sentence-in" : ""}`}
              >
                {sentence}
              </span>
            </Fragment>
          );
        })
      ) : (
        frame.text
      )}
    </span>
  );
}
