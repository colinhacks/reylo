select id, ean, upc from stock where ean = any(:eans)
