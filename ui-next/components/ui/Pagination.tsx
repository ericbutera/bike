"use client";

export default function Pagination({
  page,
  perPage,
  total,
  onPageChange,
  className = "",
  visiblePages = 5,
}: {
  page: number;
  perPage: number;
  total: number;
  onPageChange: (page: number) => void;
  className?: string;
  visiblePages?: number;
}) {
  const totalPages = Math.max(1, Math.ceil(total / perPage));
  if (totalPages <= 1) return null;

  const handlePageClick = (nextPage: number) => {
    if (nextPage !== page && nextPage >= 1 && nextPage <= totalPages) {
      onPageChange(nextPage);
    }
  };

  const pagesToShow = Math.max(1, Math.min(visiblePages, totalPages));
  let start = page - Math.floor(pagesToShow / 2);
  if (start < 1) start = 1;
  let end = start + pagesToShow - 1;
  if (end > totalPages) {
    end = totalPages;
    start = Math.max(1, end - pagesToShow + 1);
  }

  return (
    <div className={`flex justify-center ${className}`}>
      <div className="join">
        <button
          aria-label="Previous page"
          className="join-item btn"
          disabled={page === 1}
          onClick={() => handlePageClick(page - 1)}
        >
          «
        </button>
        {Array.from(
          { length: end - start + 1 },
          (_, index) => start + index,
        ).map((pageNumber) => (
          <button
            aria-current={pageNumber === page ? "page" : undefined}
            className={`join-item btn ${pageNumber === page ? "btn-active" : ""}`}
            key={pageNumber}
            onClick={() => handlePageClick(pageNumber)}
          >
            {pageNumber}
          </button>
        ))}
        <button
          aria-label="Next page"
          className="join-item btn"
          disabled={page === totalPages}
          onClick={() => handlePageClick(page + 1)}
        >
          »
        </button>
      </div>
    </div>
  );
}
