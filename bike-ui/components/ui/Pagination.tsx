"use client";

export default function Pagination({
  page,
  perPage = 1,
  total,
  cursor,
  disabled = false,
  onPageChange,
  className = "",
  visiblePages = 5,
}: {
  page: number;
  perPage?: number;
  total?: number;
  cursor?: { knownPages: number; hasNextPage: boolean };
  disabled?: boolean;
  onPageChange: (page: number) => void;
  className?: string;
  visiblePages?: number;
}) {
  const totalPages = cursor
    ? Math.max(cursor.knownPages, page + Number(cursor.hasNextPage))
    : Math.max(1, Math.ceil((total ?? 0) / perPage));
  if (totalPages <= 1) return null;

  const handlePageClick = (nextPage: number) => {
    if (nextPage !== page && nextPage >= 1 && nextPage <= totalPages) {
      onPageChange(nextPage);
    }
  };

  const pagesToShow = Math.max(1, Math.min(visiblePages, totalPages));
  const hasHiddenPages = pagesToShow < totalPages;
  let start = page - Math.floor(pagesToShow / 2);
  if (start < 1) start = 1;
  let end = start + pagesToShow - 1;
  if (end > totalPages) {
    end = totalPages;
    start = Math.max(1, end - pagesToShow + 1);
  }

  return (
    <div className={`flex justify-center ${className}`}>
      <fieldset disabled={disabled} className="join" aria-label="Pagination">
        {hasHiddenPages && page > 1 ? (
          <>
            <button
              type="button"
              aria-label="First page"
              className="join-item btn"
              onClick={() => handlePageClick(1)}
            >
              «
            </button>
            <button
              type="button"
              aria-label="Previous page"
              className="join-item btn"
              onClick={() => handlePageClick(page - 1)}
            >
              ‹
            </button>
          </>
        ) : null}
        {Array.from(
          { length: end - start + 1 },
          (_, index) => start + index,
        ).map((pageNumber) => (
          <button
            type="button"
            aria-current={pageNumber === page ? "page" : undefined}
            className={`join-item btn ${pageNumber === page ? "btn-active" : ""}`}
            key={pageNumber}
            onClick={() => handlePageClick(pageNumber)}
          >
            {pageNumber}
          </button>
        ))}
        {hasHiddenPages && page < totalPages ? (
          <>
            <button
              type="button"
              aria-label="Next page"
              className="join-item btn"
              onClick={() => handlePageClick(page + 1)}
            >
              ›
            </button>
            {!cursor ? (
              <button
                type="button"
                aria-label="Last page"
                className="join-item btn"
                onClick={() => handlePageClick(totalPages)}
              >
                »
              </button>
            ) : null}
          </>
        ) : null}
      </fieldset>
    </div>
  );
}
