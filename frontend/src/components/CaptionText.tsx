import { Fragment } from "react";
import type { CaptionFrame } from "../captions";
export function CaptionText({ frame }: { frame?: CaptionFrame }) {
  if (!frame?.text) return null;
  return (
    <span className="caption-content">
      {frame.livePartialText ? (
        <>
          {frame.committedText && (
            <span className="caption-final">{frame.committedText} </span>
          )}
          <span className="caption-partial">{frame.livePartialText}</span>
        </>
      ) : frame.sentences.length ? (
        frame.sentences.map((sentence, index) => (
          <Fragment key={index}>
            {index > 0 ? " " : ""}
            <span
              className={`caption-sentence${frame.animateNewest && index === frame.sentences.length - 1 ? " caption-sentence-in" : ""}`}
            >
              {sentence}
            </span>
          </Fragment>
        ))
      ) : (
        frame.text
      )}
    </span>
  );
}
