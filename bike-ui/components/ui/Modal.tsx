"use client";

import { useEffect, useId, useRef, type ReactNode } from "react";

export default function Modal({
  title,
  onClose,
  className = "max-w-3xl",
  children,
}: {
  title: ReactNode;
  onClose: () => void;
  className?: string;
  children: ReactNode;
}) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  return (
    <dialog
      ref={dialogRef}
      className="modal"
      aria-labelledby={titleId}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <div className={`modal-box ${className}`}>
        <h3 id={titleId} className="flex items-center gap-2 text-lg font-bold">
          {title}
        </h3>
        {children}
        <div className="modal-action">
          <button className="btn" type="button" onClick={onClose} autoFocus>
            Close
          </button>
        </div>
      </div>
      <button
        type="button"
        className="modal-backdrop"
        aria-label="Close dialog"
        onClick={onClose}
      />
    </dialog>
  );
}
