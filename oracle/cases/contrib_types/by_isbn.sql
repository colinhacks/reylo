select id, isbn13::text as printed, isn_weak() as weak, is_valid(isbn13) as valid, make_valid(isbn13) as fixed from stock where isbn13 = :isbn
