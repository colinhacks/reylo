/*
 * libpg_query's deparser over a tree already in nodes (src/raw.rs builds them from the
 * engine's tree), as pg_query_deparse_protobuf_opts runs it over the nodes it decodes:
 * default options, "; " between statements, and an error caught here, in C, never unwound
 * through the caller's frames.
 */
#include "pg_query.h"
#include "pg_query_internal.h"
#include "postgres_deparse.h"

#include "postgres.h"
#include "lib/stringinfo.h"
#include "nodes/parsenodes.h"
#include "nodes/pg_list.h"

#include <stdlib.h>
#include <string.h>

PgQueryDeparseResult
reylo_deparse_nodes(List *stmts)
{
	PgQueryDeparseResult result = {0};
	MemoryContext ctx = pg_query_enter_memory_context();

	PG_TRY();
	{
		StringInfoData str;
		PostgresDeparseOpts opts;
		ListCell   *lc;

		MemSet(&opts, 0, sizeof(PostgresDeparseOpts));
		initStringInfo(&str);
		foreach(lc, stmts)
		{
			deparseRawStmtOpts(&str, lfirst_node(RawStmt, lc), &opts);
			if (lnext(stmts, lc) != NULL)
				appendStringInfoString(&str, "; ");
		}
		result.query = strdup(str.data);
	}
	PG_CATCH();
	{
		ErrorData  *error_data;
		PgQueryError *error;

		MemoryContextSwitchTo(ctx);
		error_data = CopyErrorData();
		error = malloc(sizeof(PgQueryError));
		error->message = strdup(error_data->message);
		error->filename = strdup(error_data->filename);
		error->funcname = strdup(error_data->funcname);
		error->context = NULL;
		error->lineno = error_data->lineno;
		error->cursorpos = error_data->cursorpos;
		result.error = error;
		FlushErrorState();
	}
	PG_END_TRY();

	pg_query_exit_memory_context(ctx);
	return result;
}
