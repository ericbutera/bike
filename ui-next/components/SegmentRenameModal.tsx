"use client";

import { faPen } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useState } from "react";
import toast from "react-hot-toast";
import { type Segment, useUpdateSegment } from "../lib/queries";

type SegmentRenameModalProps = {
  segment: Segment;
};

export default function SegmentRenameModal({
  segment,
}: SegmentRenameModalProps) {
  const updateSegmentMutation = useUpdateSegment();
  const [isOpen, setIsOpen] = useState(false);
  const [draftTitle, setDraftTitle] = useState(segment.title);

  function open() {
    setDraftTitle(segment.title);
    setIsOpen(true);
  }

  function close() {
    if (updateSegmentMutation.isPending) {
      return;
    }

    setIsOpen(false);
    setDraftTitle(segment.title);
  }

  async function save() {
    const title = draftTitle.trim();

    if (!title) {
      toast.error("Segment name is required.");
      return;
    }

    if (title === segment.title) {
      close();
      return;
    }

    try {
      const updatedSegment = await updateSegmentMutation.updateAsync({
        id: segment.id,
        title,
      });

      toast.success(`Saved ${updatedSegment.title}.`);
      setIsOpen(false);
      setDraftTitle(updatedSegment.title);
    } catch {
      // Mutation errors are surfaced by the app-level React Query handler.
    }
  }

  return (
    <>
      <button
        type="button"
        className="btn btn-ghost btn-xs btn-square text-base-content/55"
        aria-label={`Rename ${segment.title}`}
        disabled={updateSegmentMutation.isPending}
        onClick={open}
      >
        <FontAwesomeIcon icon={faPen} className="h-3.5 w-3.5" />
      </button>

      {isOpen ? (
        <div
          className="modal modal-open"
          role="dialog"
          aria-modal="true"
          aria-labelledby="rename-segment-title"
        >
          <div className="modal-box max-w-lg">
            <h3 id="rename-segment-title" className="text-lg font-semibold">
              Rename segment
            </h3>

            <div className="mt-4 grid gap-4">
              <label className="form-control">
                <span className="label">
                  <span className="label-text">Segment name</span>
                </span>
                <input
                  className="input input-bordered"
                  value={draftTitle}
                  onChange={(event) => {
                    setDraftTitle(event.target.value);
                  }}
                  aria-describedby="rename-segment-current-title"
                  autoFocus
                />
              </label>

              <p id="rename-segment-current-title" className="sr-only">
                Current segment name is {segment.title}.
              </p>

              <div className="modal-action mt-0">
                <button
                  type="button"
                  className="btn btn-ghost"
                  disabled={updateSegmentMutation.isPending}
                  onClick={close}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={updateSegmentMutation.isPending}
                  onClick={() => {
                    void save();
                  }}
                >
                  {updateSegmentMutation.isPending ? "Saving..." : "Save"}
                </button>
              </div>
            </div>
          </div>

          <button
            type="button"
            className="modal-backdrop"
            aria-label="Close rename segment dialog"
            disabled={updateSegmentMutation.isPending}
            onClick={close}
          />
        </div>
      ) : null}
    </>
  );
}
