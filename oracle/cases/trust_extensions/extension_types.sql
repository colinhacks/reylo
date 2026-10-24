select handle = 'a' as h, prefs ? 'k' as has, box @> box as inside, path @> path as anc, isbn < isbn as lt, span << span as left, max(handle) over () as m from users
