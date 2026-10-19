// Original Bike artwork. Wider tire rings distinguish the fat bike at small sizes.
export default function OffRoadBikeIcon({
  fatTires = false,
}: {
  fatTires?: boolean;
}) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 64 48"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      className="h-full w-full"
      aria-hidden="true"
      focusable="false"
    >
      <g strokeWidth={fatTires ? 6 : 3.5}>
        <circle cx="13" cy="33" r="9" />
        <circle cx="51" cy="33" r="9" />
      </g>
      <path
        d="m13 33 12-13 9 13H13m12-13h19L34 33m17 0L42 15h6M21 20h8"
        strokeWidth="3"
      />
      <circle cx="36" cy="6" r="4" fill="currentColor" stroke="none" />
      <path d="m27 13 9 6 9 1" strokeWidth="4.5" />
      <path d="m27 13-7 10 12 5-5 9" strokeWidth="5" />
      {fatTires ? null : <path d="m3 47 58-5" strokeWidth="2.5" />}
    </svg>
  );
}
