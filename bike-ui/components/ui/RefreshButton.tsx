import { faArrowsRotate } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";

export default function RefreshButton({
  onClick,
  disabled,
}: {
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className="btn btn-sm btn-square"
      aria-label="Refresh"
      title="Refresh"
      disabled={disabled}
      onClick={onClick}
    >
      <FontAwesomeIcon icon={faArrowsRotate} className="h-4 w-4" />
    </button>
  );
}
