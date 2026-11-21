/*
** SQLite's own resolved parse tree, as JSON, for the engine's SQLite analysis.
**
** Built as ONE translation unit with the patched amalgamation (csrc/patch-sqlite.py inserts six
** one-line hooks, five of them right after name resolution), so the SQLITE_PRIVATE internals are
** callable here. The engine gives this file the schema DDL and one statement; the hooks fire while
** SQLite prepares the statement against an in-memory replay of that schema, and the walker
** serializes what SQLite resolved: every expression with sqlite3ExprAffinity, every column
** reference with its Table, Column, NOT NULL and outer-join mark (EP_CanBeNull), the FROM items
** with their join type and cursor, the DML target columns per value position, and each table's
** columns and unique keys. The rules (function result types,
** arithmetic, the row-count proof) live in Rust; nothing here decides a type. A host that
** registers scalar functions of its own on its connection names them with their argument counts,
** and the replay registers a stub for each, so a statement calling one prepares here as it does
** there. A host that lists the functions of its own SQLite (pragma_function_list) decides which
** functions exist: one it has and this build lacks gets a stub too, and one this build has and it
** lacks is refused. libSQL's vector functions and its vector_top_k, and the functions of the sqlean
** extensions Turso Cloud loads, are on every replay, whatever the host lists (openLibsql).
*/
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

struct Parse; struct Select; struct SelectDest; struct Table; struct IdList; struct ExprList; struct Expr; struct Upsert; struct SrcList;
typedef void (*reylo_select_fn)(struct Parse*, struct Select*, struct SelectDest*);
typedef void (*reylo_insert_fn)(struct Parse*, struct Table*, struct IdList*, struct ExprList*, struct Select*, int, struct Upsert*);
typedef void (*reylo_update_fn)(struct Parse*, struct Table*, struct ExprList*, struct Expr*, int*, struct SrcList*, int, struct Upsert*);
typedef void (*reylo_delete_fn)(struct Parse*, struct Table*, struct Expr*);
typedef void (*reylo_returning_fn)(struct Parse*, struct Table*, struct ExprList*);
typedef void (*reylo_insert_select_fn)(struct Parse*, struct Select*);
static reylo_select_fn reylo_select_hook;
static reylo_insert_fn reylo_insert_hook;
static reylo_update_fn reylo_update_hook;
static reylo_delete_fn reylo_delete_hook;
static reylo_returning_fn reylo_returning_hook;
static reylo_insert_select_fn reylo_insert_select_hook;

#include "sqlite3-patched.c"

/* ---- capture state: one statement at a time ------------------------------------------------ */

static sqlite3 *gDb;            /* the in-memory schema replay */
static char *gDdlKey;           /* the DDL it was built from */
static char *gReplayErrors;     /* what its replay reported, as openSchema's errors list them */
static sqlite3_str *gStmt;      /* the top-level SELECT, or a DML's SELECT source */
static sqlite3_str *gDml;       /* the INSERT / UPDATE / DELETE */
static sqlite3_str *gReturning; /* the RETURNING list */
static int gCaptured;           /* the first top-level hook has fired */
static Select *gInsertSelect;   /* the SELECT or VALUES that feeds the INSERT, about to be coded */
static Table *gTables[128];
static int gnTables;

/* A table of the schema the statement reads. A WITH query's table is ephemeral, as a subquery's
** is: the statement owns it and frees it as the prepare ends, before tables() reads the list. */
static void tableSeen(Table *pTab){
  int i;
  if( pTab==0 || IsView(pTab) || IsVirtual(pTab) || (pTab->tabFlags & TF_Ephemeral) ) return;
  for(i=0; i<gnTables; i++) if( gTables[i]==pTab ) return;
  if( gnTables<128 ) gTables[gnTables++] = pTab;
}

/* ---- JSON helpers -------------------------------------------------------------------------- */

static void js(sqlite3_str *s, const char *z){
  if( z==0 ){ sqlite3_str_appendall(s, "null"); return; }
  sqlite3_str_appendchar(s, 1, '"');
  for(; *z; z++){
    unsigned char c = (unsigned char)*z;
    switch( c ){
      case '"': sqlite3_str_appendall(s, "\\\""); break;
      case '\\': sqlite3_str_appendall(s, "\\\\"); break;
      case '\n': sqlite3_str_appendall(s, "\\n"); break;
      case '\r': sqlite3_str_appendall(s, "\\r"); break;
      case '\t': sqlite3_str_appendall(s, "\\t"); break;
      default:
        if( c<0x20 ) sqlite3_str_appendf(s, "\\u%04x", c);
        else sqlite3_str_appendchar(s, 1, (char)c);
    }
  }
  sqlite3_str_appendchar(s, 1, '"');
}
static void key(sqlite3_str *s, const char *k){ js(s, k); sqlite3_str_appendchar(s, 1, ':'); }
static void kv_str(sqlite3_str *s, const char *k, const char *v){ key(s, k); js(s, v); sqlite3_str_appendchar(s, 1, ','); }
static void kv_int(sqlite3_str *s, const char *k, int v){ key(s, k); sqlite3_str_appendf(s, "%d,", v); }
static void kv_bool(sqlite3_str *s, const char *k, int v){ key(s, k); sqlite3_str_appendall(s, v ? "true," : "false,"); }
#ifndef SQLITE_OMIT_WINDOWFUNC
/* A window frame's bound by the parser's token for it. */
static const char *frameBound(u8 e){
  switch( e ){
    case TK_UNBOUNDED: return "unbounded";
    case TK_CURRENT: return "current";
    case TK_PRECEDING: return "preceding";
    case TK_FOLLOWING: return "following";
    default: return "";
  }
}
#endif
/* Closes an object whose last member left a trailing comma. */
static void closeObj(sqlite3_str *s){
  int n = sqlite3_str_length(s);
  char *z = sqlite3_str_value(s);
  if( n>0 && z[n-1]==',' ) z[n-1] = '}'; else sqlite3_str_appendchar(s, 1, '}');
}
static void closeArr(sqlite3_str *s){
  int n = sqlite3_str_length(s);
  char *z = sqlite3_str_value(s);
  if( n>0 && z[n-1]==',' ) z[n-1] = ']'; else sqlite3_str_appendchar(s, 1, ']');
}

static const char *affName(char a){
  switch( a ){
    case SQLITE_AFF_BLOB: return "blob";
    case SQLITE_AFF_TEXT: return "text";
    case SQLITE_AFF_NUMERIC: return "numeric";
    case SQLITE_AFF_INTEGER: return "integer";
    case SQLITE_AFF_REAL: return "real";
    case SQLITE_AFF_FLEXNUM: return "numeric";
    default: return "none";
  }
}
static const char *opName(int op){
  switch( op ){
    case TK_COLUMN: case TK_AGG_COLUMN: case TK_TRIGGER: return "column";
    case TK_VARIABLE: return "variable"; case TK_INTEGER: return "integer"; case TK_FLOAT: return "float";
    case TK_STRING: return "string"; case TK_BLOB: return "blob"; case TK_NULL: return "null"; case TK_TRUEFALSE: return "truefalse";
    case TK_FUNCTION: case TK_AGG_FUNCTION: return "function";
    case TK_CAST: return "cast"; case TK_CONCAT: return "||"; case TK_PLUS: return "+"; case TK_MINUS: return "-"; case TK_STAR: return "*";
    case TK_SLASH: return "/"; case TK_REM: return "%"; case TK_EQ: return "="; case TK_NE: return "!="; case TK_LT: return "<"; case TK_LE: return "<=";
    case TK_GT: return ">"; case TK_GE: return ">="; case TK_IS: return "is"; case TK_ISNOT: return "is not"; case TK_IN: return "in"; case TK_BETWEEN: return "between";
    case TK_AND: return "and"; case TK_OR: return "or"; case TK_NOT: return "not"; case TK_BITAND: return "&"; case TK_BITOR: return "|";
    case TK_LSHIFT: return "<<"; case TK_RSHIFT: return ">>"; case TK_BITNOT: return "~"; case TK_UMINUS: return "neg"; case TK_UPLUS: return "pos";
    case TK_ISNULL: return "isnull"; case TK_NOTNULL: return "notnull"; case TK_SELECT: return "select"; case TK_EXISTS: return "exists";
    case TK_CASE: return "case"; case TK_COLLATE: return "collate"; case TK_VECTOR: return "vector"; case TK_SELECT_COLUMN: return "select_column";
    case TK_IF_NULL_ROW: return "if_null_row"; case TK_REGISTER: return "register"; case TK_ID: return "id"; case TK_ROW: return "row";
    default: return "other";
  }
}

static void selectJson(sqlite3_str *s, Select *p);
static const char *vectorTopKTable(SrcItem *pItem);
static void expr(sqlite3_str *s, Expr *p, Select *pSel);

static void exprList(sqlite3_str *s, ExprList *pList, Select *pSel){
  int i;
  sqlite3_str_appendchar(s, 1, '[');
  for(i=0; pList && i<pList->nExpr; i++){ expr(s, pList->a[i].pExpr, pSel); sqlite3_str_appendchar(s, 1, ','); }
  closeArr(s);
}

static int outerCursor(Select *pSel, int iCursor){
  int i;
  if( pSel==0 || pSel->pSrc==0 ) return 0;
  for(i=0; i<pSel->pSrc->nSrc; i++){
    SrcItem *pItem = &pSel->pSrc->a[i];
    if( pItem->iCursor==iCursor ) return (pItem->fg.jointype & (JT_LEFT|JT_LTORJ))!=0;
  }
  return 0;
}

/* The name a statement reads a rowid table's rowid by: "rowid", "_rowid_" or "oid", the first no
** declared column has, since a declared column of that name hides the rowid. 0 when all three do. */
static const char *rowidName(Table *pTab){
  static const char *azNames[] = { "rowid", "_rowid_", "oid" };
  int i, j;
  for(i=0; i<3; i++){
    for(j=0; j<pTab->nCol && sqlite3StrICmp(pTab->aCol[j].zCnName, azNames[i])!=0; j++){}
    if( j==pTab->nCol ) return azNames[i];
  }
  return 0;
}

/* Whether a generated column's expression calls a function or reads another generated column.
** SQLite computes a VIRTUAL column's expression where the column is read, so the JSON subtype a
** function there sets (`as (json_object('a', a)) virtual`) is still on the value read. */
static int callsOrReadsGenerated(Table *pTab, Expr *p){
  int i;
  if( p==0 ) return 0;
  if( p->op==TK_FUNCTION || p->op==TK_AGG_FUNCTION || ExprUseXSelect(p) ) return 1;
  if( p->op==TK_COLUMN && p->iColumn>=0 && p->iColumn<pTab->nCol && (pTab->aCol[p->iColumn].colFlags & COLFLAG_GENERATED) ) return 1;
  if( callsOrReadsGenerated(pTab, p->pLeft) || callsOrReadsGenerated(pTab, p->pRight) ) return 1;
  for(i=0; ExprUseXList(p) && p->x.pList && i<p->x.pList->nExpr; i++){
    if( callsOrReadsGenerated(pTab, p->x.pList->a[i].pExpr) ) return 1;
  }
  return 0;
}

/* The provenance members of a column reference (the object is already open). One SQLite resolved
** to no Table has a null "table" and no member after it. */
static void columnRef(sqlite3_str *s, Expr *p, Select *pSel){
  Table *pTab = ExprUseYTab(p) ? p->y.pTab : 0;
  int iCol = p->iColumn;
  kv_int(s, "cursor", p->iTable);
  kv_int(s, "index", iCol);
  if( pTab==0 ){ kv_str(s, "table", 0); return; }
  tableSeen(pTab);
  kv_str(s, "table", pTab->zName);
  if( iCol<0 ) iCol = pTab->iPKey;
  if( iCol<0 ){
    kv_str(s, "column", rowidName(pTab) ? rowidName(pTab) : "rowid");
    kv_str(s, "affinity", "integer");
    kv_bool(s, "notNull", 1);
    kv_bool(s, "rowid", 1);
    kv_bool(s, "generated", 0);
    kv_bool(s, "subtype", 0);
  }else{
    Column *pCol = &pTab->aCol[iCol];
    kv_str(s, "column", pCol->zCnName);
    kv_str(s, "declared", sqlite3ColumnType(pCol, ""));
    kv_str(s, "affinity", affName(pCol->affinity));
    /* The collation a comparison with the column uses, where the column decides it. */
    kv_str(s, "coll", sqlite3ColumnColl(pCol) ? sqlite3ColumnColl(pCol) : sqlite3StrBINARY);
    kv_bool(s, "notNull", pCol->notNull!=0);
    kv_bool(s, "rowid", p->iColumn<0 || iCol==pTab->iPKey);
    kv_bool(s, "generated", (pCol->colFlags & COLFLAG_GENERATED)!=0);
    kv_bool(s, "subtype", (pCol->colFlags & COLFLAG_VIRTUAL) && callsOrReadsGenerated(pTab, sqlite3ColumnExpr(pTab, pCol)));
  }
  kv_bool(s, "outer", ExprHasProperty(p, EP_CanBeNull) || (p->op==TK_COLUMN && outerCursor(pSel, p->iTable)));
  kv_bool(s, "strict", (pTab->tabFlags & TF_Strict)!=0);
  kv_bool(s, "view", IsView(pTab));
}

/* A row assigned fewer values than it has columns (`SET (a, b, c) = (SELECT x, y)`) is SQLite's
** error when it codes the statement, after the hooks: the field past the values has no expression
** for sqlite3ExprAffinity to read. */
static int missingField(Expr *p){
  return p->op==TK_SELECT_COLUMN && p->pLeft && ExprUseXSelect(p->pLeft) && p->pLeft->x.pSelect
      && (p->pLeft->x.pSelect->pEList==0 || p->iColumn>=p->pLeft->x.pSelect->pEList->nExpr);
}
static void expr(sqlite3_str *s, Expr *p, Select *pSel){
  int op;
  if( p==0 ){ sqlite3_str_appendall(s, "null"); return; }
  op = p->op;
  if( op==TK_REGISTER ) op = p->op2;
  sqlite3_str_appendchar(s, 1, '{');
  kv_str(s, "op", opName(op));
  kv_str(s, "aff", affName(missingField(p) ? SQLITE_AFF_NONE : sqlite3ExprAffinity(p)));
  /* SQLite moves every ON term into the WHERE; one from an outer join's ON says so, and names the
  ** cursor of the join's right side, which the term restricts without removing a row. */
  kv_bool(s, "outerOn", ExprHasProperty(p, EP_OuterON));
  if( ExprHasProperty(p, EP_OuterON) ) kv_int(s, "join", p->w.iJoin);
  /* One from an inner join's ON, which a RIGHT or FULL JOIN after it does not apply to the rows it
  ** null-extends. */
  kv_bool(s, "innerOn", ExprHasProperty(p, EP_InnerON));
  switch( op ){
    case TK_COLUMN: case TK_AGG_COLUMN: case TK_TRIGGER:
      columnRef(s, p, pSel);
      /* RETURNING reads the row the statement wrote from registers, not from a cursor. */
      kv_bool(s, "register", p->op==TK_REGISTER);
      break;
    case TK_VARIABLE:
      if( ExprHasProperty(p, EP_IntValue) ){ key(s, "name"); sqlite3_str_appendf(s, "\"?%d\",", p->iColumn); }
      else kv_str(s, "name", p->u.zToken);
      kv_int(s, "index", p->iColumn);
      break;
    case TK_INTEGER:
      if( ExprHasProperty(p, EP_IntValue) ){ key(s, "value"); sqlite3_str_appendf(s, "\"%d\",", p->u.iValue); }
      else kv_str(s, "value", p->u.zToken);
      break;
    case TK_FLOAT: case TK_STRING: case TK_BLOB: case TK_TRUEFALSE: case TK_ID:
      kv_str(s, "value", ExprHasProperty(p, EP_IntValue) ? "" : p->u.zToken);
      break;
    case TK_FUNCTION: case TK_AGG_FUNCTION: {
      char *zName = sqlite3_mprintf("%s", ExprHasProperty(p, EP_IntValue) ? "" : p->u.zToken);
      char *z;
      for(z=zName; z && *z; z++) *z = (char)sqlite3Tolower(*z);
      kv_str(s, "name", zName);
      sqlite3_free(zName);
      kv_bool(s, "agg", p->op==TK_AGG_FUNCTION);
      kv_bool(s, "window", ExprHasProperty(p, EP_WinFunc));
      /* A FILTER alone is held as a window too; `over` is a call with an OVER clause. */
      kv_bool(s, "over", IsWindowFunc(p));
      kv_bool(s, "distinct", ExprHasProperty(p, EP_Distinct));
      key(s, "args");
      if( !ExprHasProperty(p, EP_TokenOnly|EP_Reduced) && ExprUseXList(p) ) exprList(s, p->x.pList, pSel);
      else sqlite3_str_appendall(s, "[]");
      sqlite3_str_appendchar(s, 1, ',');
      /* An aggregate's own ORDER BY, and a window's PARTITION BY and ORDER BY (a named window's
      ** too, which resolving the call copied into it). */
#ifndef SQLITE_OMIT_WINDOWFUNC
      if( IsWindowFunc(p) && p->y.pWin ){
        Window *pWin = p->y.pWin;
        key(s, "partitionBy"); exprList(s, pWin->pPartition, pSel); sqlite3_str_appendchar(s, 1, ',');
        key(s, "orderBy"); exprList(s, pWin->pOrderBy, pSel); sqlite3_str_appendchar(s, 1, ',');
        /* The frame as resolving the call left it, a named window's copied in: where it starts and
        ** ends (a start `unbounded` is UNBOUNDED PRECEDING, an end UNBOUNDED FOLLOWING), what it
        ** excludes, and whether a FILTER narrows the rows it aggregates. */
        kv_str(s, "frameStart", frameBound(pWin->eStart));
        kv_str(s, "frameEnd", frameBound(pWin->eEnd));
        kv_str(s, "exclude", pWin->eExclude==TK_CURRENT ? "current" : pWin->eExclude==TK_GROUP ? "group" : pWin->eExclude==TK_TIES ? "ties" : "no");
        kv_bool(s, "filter", pWin->pFilter!=0);
      }else
#endif
      if( !ExprHasProperty(p, EP_TokenOnly|EP_Reduced) && p->pLeft && p->pLeft->op==TK_ORDER && ExprUseXList(p->pLeft) ){
        key(s, "orderBy"); exprList(s, p->pLeft->x.pList, pSel); sqlite3_str_appendchar(s, 1, ',');
      }
      break;
    }
    case TK_CAST:
      kv_str(s, "affinity", affName(sqlite3AffinityType(ExprHasProperty(p, EP_IntValue) ? "" : p->u.zToken, 0)));
      /* fall through: the operand is pLeft */
      key(s, "left"); expr(s, ExprHasProperty(p, EP_TokenOnly) ? 0 : p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ',');
      break;
    case TK_IN:
      key(s, "left"); expr(s, p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ',');
      if( ExprUseXSelect(p) ){ key(s, "select"); selectJson(s, p->x.pSelect); }
      else { key(s, "list"); exprList(s, p->x.pList, pSel); }
      sqlite3_str_appendchar(s, 1, ',');
      break;
    case TK_BETWEEN: case TK_CASE: case TK_VECTOR:
      key(s, "left"); expr(s, p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ',');
      key(s, "list"); if( ExprUseXList(p) ) exprList(s, p->x.pList, pSel); else sqlite3_str_appendall(s, "[]");
      sqlite3_str_appendchar(s, 1, ',');
      break;
    case TK_SELECT: case TK_EXISTS:
      key(s, "select"); if( ExprUseXSelect(p) ) selectJson(s, p->x.pSelect); else sqlite3_str_appendall(s, "null");
      sqlite3_str_appendchar(s, 1, ',');
      break;
    case TK_SELECT_COLUMN:
      kv_int(s, "index", p->iColumn);
      key(s, "left"); expr(s, p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ',');
      break;
    default:
      if( !ExprHasProperty(p, EP_TokenOnly) ){
        if( p->pLeft ){ key(s, "left"); expr(s, p->pLeft, pSel); sqlite3_str_appendchar(s, 1, ','); }
        if( p->pRight ){ key(s, "right"); expr(s, p->pRight, pSel); sqlite3_str_appendchar(s, 1, ','); }
      }
      break;
  }
  closeObj(s);
}

static void selectJson(sqlite3_str *s, Select *pTop){
  Select *p = pTop;
  int i, nMore = 0;
  if( p==0 ){ sqlite3_str_appendall(s, "null"); return; }
  /* A multi-row VALUES is a chain of one-row SELECTs under UNION ALL, the last row on top. Its
  ** first row stands for it here, with the rows after it in "rows", so a list of any length nests
  ** nothing; an ORDER BY or a LIMIT is on the top row, as on any compound. */
  while( (p->selFlags & SF_Values) && p->op==TK_ALL && p->pPrior && (p->pPrior->selFlags & SF_Values) ){ p = p->pPrior; nMore++; }
  sqlite3_str_appendchar(s, 1, '{');
  key(s, "columns"); sqlite3_str_appendchar(s, 1, '[');
  for(i=0; p->pEList && i<p->pEList->nExpr; i++){
    struct ExprList_item *pItem = &p->pEList->a[i];
    sqlite3_str_appendchar(s, 1, '{');
    kv_str(s, "name", pItem->zEName);
    kv_bool(s, "named", pItem->fg.eEName==ENAME_NAME);
    key(s, "expr"); expr(s, pItem->pExpr, p); sqlite3_str_appendchar(s, 1, ',');
    closeObj(s); sqlite3_str_appendchar(s, 1, ',');
  }
  closeArr(s); sqlite3_str_appendchar(s, 1, ',');
  if( nMore ){
    Select **apRow = sqlite3_malloc64(sizeof(Select*)*(sqlite3_uint64)nMore);
    Select *q;
    key(s, "rows"); sqlite3_str_appendchar(s, 1, '[');
    for(i=nMore, q=pTop; apRow && i>0; q=q->pPrior) apRow[--i] = q;
    for(i=0; apRow && i<nMore; i++){ exprList(s, apRow[i]->pEList, apRow[i]); sqlite3_str_appendchar(s, 1, ','); }
    sqlite3_free(apRow);
    closeArr(s); sqlite3_str_appendchar(s, 1, ',');
  }
  key(s, "from"); sqlite3_str_appendchar(s, 1, '[');
  for(i=0; p->pSrc && i<p->pSrc->nSrc; i++){
    SrcItem *pItem = &p->pSrc->a[i];
    sqlite3_str_appendchar(s, 1, '{');
    kv_int(s, "cursor", pItem->iCursor);
    /* A table by the name its DDL spells, which "tables" is keyed by: SQLite's names fold ASCII
    ** case, so the statement may spell it another way. A table-valued function by its module's. */
    kv_str(s, "table", pItem->pSTab && !pItem->fg.isSubquery ? pItem->pSTab->zName : pItem->zName);
    kv_str(s, "alias", pItem->zAlias);
    kv_bool(s, "outer", (pItem->fg.jointype & (JT_LEFT|JT_LTORJ))!=0);
    kv_bool(s, "right", (pItem->fg.jointype & JT_RIGHT)!=0);
    kv_bool(s, "leftOfRight", (pItem->fg.jointype & JT_LTORJ)!=0);
    kv_bool(s, "tabFunc", pItem->fg.isTabFunc);
    key(s, "args"); exprList(s, pItem->fg.isTabFunc ? pItem->u1.pFuncArg : 0, p); sqlite3_str_appendchar(s, 1, ',');
    kv_str(s, "vectorTable", vectorTopKTable(pItem));
    kv_bool(s, "cte", pItem->fg.isCte);
    /* A recursive WITH query's read of itself, inside the query: it has no subquery of its own. */
    kv_bool(s, "recursive", pItem->fg.isRecursive);
    if( pItem->pSTab ) tableSeen(pItem->pSTab);
    kv_bool(s, "view", pItem->pSTab && IsView(pItem->pSTab));
    kv_bool(s, "virtual", pItem->pSTab && IsVirtual(pItem->pSTab));
    key(s, "subquery");
    if( pItem->fg.isSubquery && pItem->u4.pSubq ) selectJson(s, pItem->u4.pSubq->pSelect); else sqlite3_str_appendall(s, "null");
    sqlite3_str_appendchar(s, 1, ',');
    key(s, "on"); if( !pItem->fg.isUsing && pItem->u3.pOn ) expr(s, pItem->u3.pOn, p); else sqlite3_str_appendall(s, "null");
    sqlite3_str_appendchar(s, 1, ',');
    closeObj(s); sqlite3_str_appendchar(s, 1, ',');
  }
  closeArr(s); sqlite3_str_appendchar(s, 1, ',');
  key(s, "where"); expr(s, p->pWhere, p); sqlite3_str_appendchar(s, 1, ',');
  key(s, "groupBy"); exprList(s, p->pGroupBy, p); sqlite3_str_appendchar(s, 1, ',');
  key(s, "having"); expr(s, p->pHaving, p); sqlite3_str_appendchar(s, 1, ',');
  key(s, "orderBy"); exprList(s, pTop->pOrderBy, pTop); sqlite3_str_appendchar(s, 1, ',');
  key(s, "limit"); expr(s, pTop->pLimit ? pTop->pLimit->pLeft : 0, pTop); sqlite3_str_appendchar(s, 1, ',');
  key(s, "offset"); expr(s, pTop->pLimit ? pTop->pLimit->pRight : 0, pTop); sqlite3_str_appendchar(s, 1, ',');
  kv_bool(s, "distinct", (p->selFlags & SF_Distinct)!=0);
  kv_bool(s, "aggregate", (p->selFlags & SF_Aggregate)!=0);
  kv_bool(s, "values", (p->selFlags & SF_Values)!=0);
  kv_bool(s, "window", p->pWin!=0);
  if( p->pPrior ){
    kv_str(s, "op", p->op==TK_ALL ? "union all" : p->op==TK_UNION ? "union" : p->op==TK_INTERSECT ? "intersect" : "except");
    key(s, "prior"); selectJson(s, p->pPrior); sqlite3_str_appendchar(s, 1, ',');
  }
  closeObj(s);
}

/* ---- the hooks ------------------------------------------------------------------------------- */

static int skipHook(Parse *pParse){ return pParse->nErr || pParse->db->init.busy; }

static void selectHook(Parse *pParse, Select *p, SelectDest *pDest){
  if( skipHook(pParse) ) return;
  if( p==gInsertSelect ){
    gInsertSelect = 0;
    selectJson(gStmt, p);
  }else if( !gCaptured && pDest->eDest==SRT_Output ){
    gCaptured = 1;
    selectJson(gStmt, p);
  }
}
/* SQLite codes the SELECT that feeds an INSERT before it reaches the INSERT hook: this names it,
** and the SELECT hook serializes it as resolved, before the code generator rewrites it. */
static void insertSelectHook(Parse *pParse, Select *pSelect){
  if( skipHook(pParse) || gCaptured || sqlite3_str_length(gStmt) ) return;
  gInsertSelect = pSelect;
}

static int insertColumnIndex(Table *pTab, IdList *pColumn, int i){
  int j, seen = 0;
  if( pColumn ){
    if( i>=pColumn->nId ) return -1;
    for(j=0; j<pTab->nCol; j++) if( sqlite3StrICmp(pTab->aCol[j].zCnName, pColumn->a[i].zName)==0 ) return j;
    /* The rowid by one of its names is the INTEGER PRIMARY KEY column, where the table has one. */
    return sqlite3IsRowid(pColumn->a[i].zName) ? pTab->iPKey : -1;
  }
  for(j=0; j<pTab->nCol; j++){
    if( pTab->aCol[j].colFlags & COLFLAG_NOINSERT ) continue;
    if( seen++==i ) return j;
  }
  return -1;
}
static const char *onErrorName(int onError){
  switch( onError ){
    case OE_Ignore: return "ignore"; case OE_Replace: return "replace"; case OE_Abort: return "abort";
    case OE_Rollback: return "rollback"; case OE_Fail: return "fail"; default: return "default";
  }
}
/* Every ON CONFLICT clause of an upsert, in order: SQLite tries each against the conflict. */
static void upsert(sqlite3_str *s, Upsert *pUpsert){
  int i;
  sqlite3_str_appendchar(s, 1, '[');
  for(; pUpsert; pUpsert=pUpsert->pNextUpsert){
    sqlite3_str_appendchar(s, 1, '{');
    kv_bool(s, "doUpdate", pUpsert->isDoUpdate);
    key(s, "set"); sqlite3_str_appendchar(s, 1, '[');
    for(i=0; pUpsert->pUpsertSet && i<pUpsert->pUpsertSet->nExpr; i++){
      sqlite3_str_appendchar(s, 1, '{');
      kv_str(s, "column", pUpsert->pUpsertSet->a[i].zEName);
      key(s, "expr"); expr(s, pUpsert->pUpsertSet->a[i].pExpr, 0); sqlite3_str_appendchar(s, 1, ',');
      closeObj(s); sqlite3_str_appendchar(s, 1, ',');
    }
    closeArr(s); sqlite3_str_appendchar(s, 1, ',');
    key(s, "where"); expr(s, pUpsert->pUpsertWhere, 0); sqlite3_str_appendchar(s, 1, ',');
    closeObj(s); sqlite3_str_appendchar(s, 1, ',');
  }
  closeArr(s);
}
static void insertHook(Parse *pParse, Table *pTab, IdList *pColumn, ExprList *pList, Select *pSelect, int onError, Upsert *pUpsert){
  int i, n;
  if( skipHook(pParse) || gCaptured ) return;
  gCaptured = 1;
  tableSeen(pTab);
  sqlite3_str_appendchar(gDml, 1, '{');
  kv_str(gDml, "kind", "insert");
  kv_str(gDml, "table", pTab->zName);
  kv_str(gDml, "onError", onErrorName(onError));
  key(gDml, "upserts"); upsert(gDml, pUpsert); sqlite3_str_appendchar(gDml, 1, ',');
  n = pList ? pList->nExpr : pSelect && pSelect->pEList ? pSelect->pEList->nExpr : 0;
  key(gDml, "columns"); sqlite3_str_appendchar(gDml, 1, '[');
  for(i=0; i<n; i++){
    int j = insertColumnIndex(pTab, pColumn, i);
    js(gDml, j>=0 ? pTab->aCol[j].zCnName : 0); sqlite3_str_appendchar(gDml, 1, ',');
  }
  closeArr(gDml); sqlite3_str_appendchar(gDml, 1, ',');
  key(gDml, "values"); if( pList ) exprList(gDml, pList, 0); else sqlite3_str_appendall(gDml, "null");
  sqlite3_str_appendchar(gDml, 1, ',');
  if( pSelect ){
    int rows = 0;
    Select *q;
    if( pSelect->selFlags & SF_Values ){ for(q=pSelect; q; q=q->pPrior) rows++; }
    kv_int(gDml, "rows", (pSelect->selFlags & SF_Values) ? rows : -1);
  }else{
    kv_int(gDml, "rows", 1);
  }
  closeObj(gDml);
}
static void updateHook(Parse *pParse, Table *pTab, ExprList *pChanges, Expr *pWhere, int *aXRef, SrcList *pTabList, int onError, Upsert *pUpsert){
  int i, j;
  (void)pTabList;
  if( skipHook(pParse) || gCaptured ) return;
  gCaptured = 1;
  tableSeen(pTab);
  sqlite3_str_appendchar(gDml, 1, '{');
  kv_str(gDml, "kind", pUpsert ? "upsert" : "update");
  kv_str(gDml, "table", pTab->zName);
  kv_str(gDml, "onError", onErrorName(onError));
  key(gDml, "set"); sqlite3_str_appendchar(gDml, 1, '[');
  for(i=0; pChanges && i<pChanges->nExpr; i++){
    int col = -1;
    for(j=0; j<pTab->nCol; j++) if( aXRef[j]==i ) col = j;
    sqlite3_str_appendchar(gDml, 1, '{');
    kv_str(gDml, "column", col>=0 ? pTab->aCol[col].zCnName : 0);
    key(gDml, "expr"); expr(gDml, pChanges->a[i].pExpr, 0); sqlite3_str_appendchar(gDml, 1, ',');
    closeObj(gDml); sqlite3_str_appendchar(gDml, 1, ',');
  }
  closeArr(gDml); sqlite3_str_appendchar(gDml, 1, ',');
  key(gDml, "where"); expr(gDml, pWhere, 0); sqlite3_str_appendchar(gDml, 1, ',');
  closeObj(gDml);
}
static void deleteHook(Parse *pParse, Table *pTab, Expr *pWhere){
  if( skipHook(pParse) || gCaptured ) return;
  gCaptured = 1;
  tableSeen(pTab);
  sqlite3_str_appendchar(gDml, 1, '{');
  kv_str(gDml, "kind", "delete");
  kv_str(gDml, "table", pTab->zName);
  key(gDml, "where"); expr(gDml, pWhere, 0); sqlite3_str_appendchar(gDml, 1, ',');
  closeObj(gDml);
}
static void returningHook(Parse *pParse, Table *pTab, ExprList *pNew){
  int i;
  if( skipHook(pParse) || sqlite3_str_length(gReturning) ) return;
  tableSeen(pTab);
  sqlite3_str_appendchar(gReturning, 1, '[');
  for(i=0; pNew && i<pNew->nExpr; i++){
    sqlite3_str_appendchar(gReturning, 1, '{');
    kv_str(gReturning, "name", pNew->a[i].zEName);
    kv_bool(gReturning, "named", pNew->a[i].fg.eEName==ENAME_NAME);
    key(gReturning, "expr"); expr(gReturning, pNew->a[i].pExpr, 0); sqlite3_str_appendchar(gReturning, 1, ',');
    closeObj(gReturning); sqlite3_str_appendchar(gReturning, 1, ',');
  }
  closeArr(gReturning);
}

/* ---- tables: columns and unique keys ------------------------------------------------------- */

/* Whether a CHECK refuses NULL in column iCol: one of its conjuncts tests the column IS NOT NULL,
** which NULL makes false where any other test of it would be NULL, and so pass. The CHECK's names
** are resolved (TK_COLUMN) once the table is built, and still names (TK_ID) before. */
static int refusesNull(Expr *p, Table *pTab, int iCol){
  Expr *x;
  if( p==0 ) return 0;
  if( p->op==TK_AND ) return refusesNull(p->pLeft, pTab, iCol) || refusesNull(p->pRight, pTab, iCol);
  if( p->op!=TK_NOTNULL ) return 0;
  for(x=p->pLeft; x && x->op==TK_COLLATE; x=x->pLeft){}
  if( x==0 ) return 0;
  if( x->op==TK_COLUMN ) return x->iColumn==iCol;
  if( x->op==TK_ID ) return sqlite3StrICmp(x->u.zToken, pTab->aCol[iCol].zCnName)==0;
  return 0;
}
static int checkedNotNull(Table *pTab, int iCol){
#ifndef SQLITE_OMIT_CHECK
  int k;
  for(k=0; pTab->pCheck && k<pTab->pCheck->nExpr; k++){
    if( refusesNull(pTab->pCheck->a[k].pExpr, pTab, iCol) ) return 1;
  }
#endif
  return 0;
}

/* Whether the table declares a constraint ON CONFLICT IGNORE (a NOT NULL, a UNIQUE or the PRIMARY
** KEY): a write that breaks it skips the row, unless the statement's own OR clause says otherwise. */
static int ignoresConflict(Table *pTab){
  Index *pIdx;
  int i;
  if( pTab->keyConf==OE_Ignore ) return 1;
  for(i=0; i<pTab->nCol; i++) if( pTab->aCol[i].notNull==OE_Ignore ) return 1;
  for(pIdx=pTab->pIndex; pIdx; pIdx=pIdx->pNext) if( pIdx->onError==OE_Ignore ) return 1;
  return 0;
}

/* Whether the table declares its PRIMARY KEY or a UNIQUE ON CONFLICT REPLACE: a write that breaks
** it deletes the row it conflicts with, unless the statement's own OR clause says otherwise. A NOT
** NULL's REPLACE writes the column's default instead, and deletes nothing. */
static int replacesConflict(Table *pTab){
  Index *pIdx;
  if( pTab->keyConf==OE_Replace ) return 1;
  for(pIdx=pTab->pIndex; pIdx; pIdx=pIdx->pNext) if( pIdx->onError==OE_Replace ) return 1;
  return 0;
}

static void tables(sqlite3_str *s){
  int t, i, k;
  sqlite3_str_appendchar(s, 1, '{');
  for(t=0; t<gnTables; t++){
    Table *pTab = gTables[t];
    Index *pIdx;
    key(s, pTab->zName); sqlite3_str_appendchar(s, 1, '{');
    kv_bool(s, "strict", (pTab->tabFlags & TF_Strict)!=0);
    kv_bool(s, "withoutRowid", (pTab->tabFlags & TF_WithoutRowid)!=0);
    kv_str(s, "rowidAlias", pTab->iPKey>=0 ? pTab->aCol[pTab->iPKey].zCnName : 0);
    kv_bool(s, "ignores", ignoresConflict(pTab));
    kv_bool(s, "replaces", replacesConflict(pTab));
    key(s, "columns"); sqlite3_str_appendchar(s, 1, '[');
    for(i=0; i<pTab->nCol; i++){
      Column *pCol = &pTab->aCol[i];
      sqlite3_str_appendchar(s, 1, '{');
      kv_str(s, "name", pCol->zCnName);
      kv_str(s, "declared", sqlite3ColumnType(pCol, ""));
      kv_str(s, "affinity", affName(pCol->affinity));
      kv_bool(s, "notNull", pCol->notNull!=0);
      kv_bool(s, "checkedNotNull", checkedNotNull(pTab, i));
      closeObj(s); sqlite3_str_appendchar(s, 1, ',');
    }
    closeArr(s); sqlite3_str_appendchar(s, 1, ',');
    /* A unique key: every unique, non-partial index over plain columns, the INTEGER PRIMARY KEY, and the rowid itself. */
    key(s, "uniqueKeys"); sqlite3_str_appendchar(s, 1, '[');
    if( pTab->iPKey>=0 ){ sqlite3_str_appendchar(s, 1, '['); js(s, pTab->aCol[pTab->iPKey].zCnName); sqlite3_str_appendall(s, "],"); }
    else if( !(pTab->tabFlags & TF_WithoutRowid) && rowidName(pTab) ){ sqlite3_str_appendchar(s, 1, '['); js(s, rowidName(pTab)); sqlite3_str_appendall(s, "],"); }
    for(pIdx=pTab->pIndex; pIdx; pIdx=pIdx->pNext){
      int ok = pIdx->onError!=OE_None && pIdx->pPartIdxWhere==0 && !pIdx->bHasExpr;
      for(k=0; ok && k<pIdx->nKeyCol; k++) if( pIdx->aiColumn[k]<0 ) ok = 0;
      /* A comparison of the column uses the column's collation: an index under another one bounds
      ** it only when the column's is BINARY, which tells every value apart (NOCASE does not). */
      for(k=0; ok && k<pIdx->nKeyCol; k++){
        const char *zColl = sqlite3ColumnColl(&pTab->aCol[pIdx->aiColumn[k]]);
        const char *zIdxColl = pIdx->azColl[k];
        if( zColl==0 ) zColl = sqlite3StrBINARY;
        if( zIdxColl==0 ) zIdxColl = sqlite3StrBINARY;
        if( sqlite3StrICmp(zColl, zIdxColl)!=0 && sqlite3StrICmp(zColl, sqlite3StrBINARY)!=0 ) ok = 0;
      }
      if( !ok ) continue;
      sqlite3_str_appendchar(s, 1, '[');
      for(k=0; k<pIdx->nKeyCol; k++){ js(s, pTab->aCol[pIdx->aiColumn[k]].zCnName); sqlite3_str_appendchar(s, 1, ','); }
      closeArr(s); sqlite3_str_appendchar(s, 1, ',');
    }
    closeArr(s); sqlite3_str_appendchar(s, 1, ',');
    closeObj(s); sqlite3_str_appendchar(s, 1, ',');
  }
  closeObj(s);
}

/* ---- the schema replay and the entry point ------------------------------------------------- */

static void installHooks(int on){
  reylo_select_hook = on ? selectHook : 0;
  reylo_insert_hook = on ? insertHook : 0;
  reylo_update_hook = on ? updateHook : 0;
  reylo_delete_hook = on ? deleteHook : 0;
  reylo_returning_hook = on ? returningHook : 0;
  reylo_insert_select_hook = on ? insertSelectHook : 0;
}

/* A host function's stand-in: the replay only prepares, so it is never called. */
static void hostFunctionStub(sqlite3_context *ctx, int argc, sqlite3_value **argv){
  (void)argc; (void)argv;
  sqlite3_result_null(ctx);
}
static void hostFinalStub(sqlite3_context *ctx){ sqlite3_result_null(ctx); }

/* ---- libSQL's own SQL ------------------------------------------------------------------------ */

/* libSQL, Turso's fork of SQLite, adds vectors: the scalar functions below, under these names and
** argument counts (libSQL 0.10.0's vector.c and vectorIndex.c), and the table-valued vector_top_k.
** Turso Cloud also loads into every connection the sqlean extensions libsql-ffi 0.10.0 carries
** (uuid, crypto, fuzzy, math, stats, text, regexp), whose functions follow, and whose
** generate_series and stats_seq are table-valued. A libSQL database file reads as SQLite's, with
** nothing in it to say whose it is, so the replay has them on every database, as it has every
** optional family of SQLite's, and the SQLite the application runs on decides: the type check also
** prepares each statement on the application's own drivers (src/sqlite-drivers.ts), and one whose
** SQLite lacks them refuses it. The analysis only prepares, so a stand-in under the same name,
** argument count, kind and flags resolves as libSQL's own does. A name this build has already, as
** sqlean's math functions, `concat` and `median`, keeps this build's function. */
#define D (SQLITE_UTF8|SQLITE_DETERMINISTIC)
#define I (SQLITE_UTF8|SQLITE_INNOCUOUS)
#define DI (SQLITE_UTF8|SQLITE_DETERMINISTIC|SQLITE_INNOCUOUS)
static const struct TursoFunction { const char *zName; int nArg; char cKind; int flags; } aTurso[] = {
  /* libSQL's vectors */
  {"vector", 1, 's', DI}, {"vector32", 1, 's', DI}, {"vector64", 1, 's', DI}, {"vector1bit", 1, 's', DI},
  {"vector8", 1, 's', DI}, {"vector16", 1, 's', DI}, {"vectorb16", 1, 's', DI}, {"vector_extract", 1, 's', DI},
  {"vector_distance_cos", 2, 's', DI}, {"vector_distance_l2", 2, 's', DI}, {"libsql_vector_idx", -1, 's', DI},
  /* sqlean's uuid */
  {"gen_random_uuid", 0, 's', I}, {"uuid4", 0, 's', I}, {"uuid7", 0, 's', I},
  {"uuid7_timestamp_ms", 1, 's', DI}, {"uuid_blob", 1, 's', DI}, {"uuid_str", 1, 's', DI},
  /* sqlean's crypto */
  {"blake3", 1, 's', DI}, {"crypto_blake3", 1, 's', DI}, {"crypto_decode", 2, 's', DI},
  {"crypto_encode", 2, 's', DI}, {"crypto_md5", 1, 's', DI}, {"crypto_sha1", 1, 's', DI},
  {"crypto_sha256", 1, 's', DI}, {"crypto_sha384", 1, 's', DI}, {"crypto_sha512", 1, 's', DI},
  {"decode", 2, 's', DI}, {"encode", 2, 's', DI}, {"md5", 1, 's', DI}, {"sha1", 1, 's', DI},
  {"sha256", 1, 's', DI}, {"sha384", 1, 's', DI}, {"sha512", 1, 's', DI},
  /* sqlean's fuzzy */
  {"caverphone", 1, 's', DI}, {"dlevenshtein", 2, 's', DI}, {"edit_distance", 2, 's', DI},
  {"fuzzy_caver", 1, 's', DI}, {"fuzzy_damlev", 2, 's', DI}, {"fuzzy_editdist", 2, 's', DI},
  {"fuzzy_hamming", 2, 's', DI}, {"fuzzy_jarowin", 2, 's', DI}, {"fuzzy_leven", 2, 's', DI},
  {"fuzzy_osadist", 2, 's', DI}, {"fuzzy_phonetic", 1, 's', DI}, {"fuzzy_rsoundex", 1, 's', DI},
  {"fuzzy_script", 1, 's', DI}, {"fuzzy_soundex", 1, 's', DI}, {"fuzzy_translit", 1, 's', DI},
  {"hamming", 2, 's', DI}, {"jaro_winkler", 2, 's', DI}, {"levenshtein", 2, 's', DI},
  {"osa_distance", 2, 's', DI}, {"phonetic_hash", 1, 's', DI}, {"rsoundex", 1, 's', DI},
  {"script_code", 1, 's', DI}, {"translit", 1, 's', DI},
  /* sqlean's math */
  {"math_acos", 1, 's', DI}, {"math_acosh", 1, 's', DI}, {"math_asin", 1, 's', DI},
  {"math_asinh", 1, 's', DI}, {"math_atan", 1, 's', DI}, {"math_atan2", 2, 's', DI},
  {"math_atanh", 1, 's', DI}, {"math_ceil", 1, 's', DI}, {"math_cos", 1, 's', DI}, {"math_cosh", 1, 's', DI},
  {"math_degrees", 1, 's', DI}, {"math_exp", 1, 's', DI}, {"math_floor", 1, 's', DI},
  {"math_ln", 1, 's', DI}, {"math_log", 1, 's', DI}, {"math_log", 2, 's', DI}, {"math_log10", 1, 's', DI},
  {"math_log2", 1, 's', DI}, {"math_mod", 2, 's', DI}, {"math_pi", 0, 's', DI}, {"math_pow", 2, 's', DI},
  {"math_radians", 1, 's', DI}, {"math_round", 1, 's', DI}, {"math_round", 2, 's', DI},
  {"math_sin", 1, 's', DI}, {"math_sinh", 1, 's', DI}, {"math_sqrt", 1, 's', DI}, {"math_tan", 1, 's', DI},
  {"math_tanh", 1, 's', DI}, {"math_trunc", 1, 's', DI},
  /* sqlean's stats */
  {"percentile_25", 1, 'a', I}, {"percentile_75", 1, 'a', I}, {"percentile_90", 1, 'a', I},
  {"percentile_95", 1, 'a', I}, {"percentile_99", 1, 'a', I}, {"stats_median", 1, 'a', I},
  {"stats_p25", 1, 'a', I}, {"stats_p75", 1, 'a', I}, {"stats_p90", 1, 'a', I}, {"stats_p95", 1, 'a', I},
  {"stats_p99", 1, 'a', I}, {"stats_perc", 2, 'a', I}, {"stats_stddev", 1, 'a', I},
  {"stats_stddev_pop", 1, 'a', I}, {"stats_stddev_samp", 1, 'a', I}, {"stats_var", 1, 'a', I},
  {"stats_var_pop", 1, 'a', I}, {"stats_var_samp", 1, 'a', I}, {"stddev", 1, 'a', I},
  {"stddev_pop", 1, 'a', I}, {"stddev_samp", 1, 'a', I}, {"var_pop", 1, 'a', I}, {"var_samp", 1, 'a', I},
  {"variance", 1, 'a', I},
  /* sqlean's text */
  {"bit_length", 1, 's', DI}, {"btrim", -1, 's', DI}, {"char_length", 1, 's', DI},
  {"character_length", 1, 's', DI}, {"left", 2, 's', DI}, {"lpad", -1, 's', DI}, {"repeat", 2, 's', DI},
  {"reverse", 1, 's', DI}, {"right", 2, 's', DI}, {"rpad", -1, 's', DI}, {"split_part", 3, 's', DI},
  {"starts_with", 2, 's', DI}, {"strpos", 2, 's', DI}, {"text_bitsize", 1, 's', DI},
  {"text_casefold", 1, 's', DI}, {"text_concat", -1, 's', DI}, {"text_contains", 2, 's', DI},
  {"text_count", 2, 's', DI}, {"text_has_prefix", 2, 's', DI}, {"text_has_suffix", 2, 's', DI},
  {"text_index", 2, 's', DI}, {"text_join", -1, 's', DI}, {"text_last_index", 2, 's', DI},
  {"text_left", 2, 's', DI}, {"text_length", 1, 's', DI}, {"text_like", 2, 's', DI},
  {"text_lower", 1, 's', DI}, {"text_lpad", -1, 's', DI}, {"text_ltrim", -1, 's', DI},
  {"text_repeat", 2, 's', DI}, {"text_replace", 3, 's', DI}, {"text_replace", 4, 's', DI},
  {"text_reverse", 1, 's', DI}, {"text_right", 2, 's', DI}, {"text_rpad", -1, 's', DI},
  {"text_rtrim", -1, 's', DI}, {"text_size", 1, 's', DI}, {"text_slice", 2, 's', DI},
  {"text_slice", 3, 's', DI}, {"text_split", 3, 's', DI}, {"text_substring", 2, 's', DI},
  {"text_substring", 3, 's', DI}, {"text_title", 1, 's', DI}, {"text_translate", 3, 's', DI},
  {"text_trim", -1, 's', DI}, {"text_upper", 1, 's', DI}, {"translate", 3, 's', DI},
  /* sqlean's regexp */
  {"regexp", 2, 's', D}, {"regexp_capture", 2, 's', D}, {"regexp_capture", 3, 's', D},
  {"regexp_like", 2, 's', D}, {"regexp_replace", 3, 's', D}, {"regexp_substr", 2, 's', D},
};
#undef D
#undef I
#undef DI

static int isTursoFunction(const char *zName){
  int i;
  for(i=0; i<(int)(sizeof(aTurso)/sizeof(aTurso[0])); i++) if( sqlite3StrICmp(aTurso[i].zName, zName)==0 ) return 1;
  return 0;
}

/* vector_top_k(index, vector, k): the rows of a libSQL vector index nearest the vector, each by
** the key of the table's row, as libSQL 0.10.0's vectorvtab.c declares it, with its planning:
** every argument is an equality constraint on a hidden column, which the plan consumes. It is
** only ever prepared here, so a scan of it has no rows. */
static int vectorTopKConnect(sqlite3 *db, void *pAux, int argc, const char *const *argv, sqlite3_vtab **ppVtab, char **pzErr){
  int rc;
  (void)pAux; (void)argc; (void)argv; (void)pzErr;
  rc = sqlite3_declare_vtab(db, "CREATE TABLE x(idx hidden, vector hidden, k hidden, id);");
  if( rc!=SQLITE_OK ) return rc;
  *ppVtab = sqlite3_malloc(sizeof(sqlite3_vtab));
  if( *ppVtab==0 ) return SQLITE_NOMEM;
  memset(*ppVtab, 0, sizeof(sqlite3_vtab));
  return SQLITE_OK;
}
static int vectorTopKDisconnect(sqlite3_vtab *pVtab){ sqlite3_free(pVtab); return SQLITE_OK; }
static int vectorTopKBestIndex(sqlite3_vtab *pVtab, sqlite3_index_info *pInfo){
  int i;
  (void)pVtab;
  pInfo->estimatedCost = 1.0;
  pInfo->estimatedRows = 100;
  pInfo->idxNum = 1;
  for(i=0; i<pInfo->nConstraint; i++){
    const struct sqlite3_index_constraint *p = &pInfo->aConstraint[i];
    if( !p->usable || p->op!=SQLITE_INDEX_CONSTRAINT_EQ || p->iColumn<0 || p->iColumn>2 ) continue;
    pInfo->aConstraintUsage[i].argvIndex = p->iColumn + 1;
    pInfo->aConstraintUsage[i].omit = 1;
  }
  return SQLITE_OK;
}
static int vectorTopKOpen(sqlite3_vtab *pVtab, sqlite3_vtab_cursor **ppCursor){
  (void)pVtab;
  *ppCursor = sqlite3_malloc(sizeof(sqlite3_vtab_cursor));
  if( *ppCursor==0 ) return SQLITE_NOMEM;
  memset(*ppCursor, 0, sizeof(sqlite3_vtab_cursor));
  return SQLITE_OK;
}
static int vectorTopKClose(sqlite3_vtab_cursor *pCursor){ sqlite3_free(pCursor); return SQLITE_OK; }
static int vectorTopKFilter(sqlite3_vtab_cursor *pCursor, int idxNum, const char *idxStr, int argc, sqlite3_value **argv){
  (void)pCursor; (void)idxNum; (void)idxStr; (void)argc; (void)argv;
  return SQLITE_OK;
}
static int vectorTopKNext(sqlite3_vtab_cursor *pCursor){ (void)pCursor; return SQLITE_OK; }
static int vectorTopKEof(sqlite3_vtab_cursor *pCursor){ (void)pCursor; return 1; }
static int vectorTopKColumn(sqlite3_vtab_cursor *pCursor, sqlite3_context *ctx, int iCol){
  (void)pCursor; (void)iCol;
  sqlite3_result_null(ctx);
  return SQLITE_OK;
}
static int vectorTopKRowid(sqlite3_vtab_cursor *pCursor, sqlite_int64 *pRowid){ (void)pCursor; *pRowid = 0; return SQLITE_OK; }
static sqlite3_module vectorTopKModule = {
  0, 0, vectorTopKConnect, vectorTopKBestIndex, vectorTopKDisconnect, 0, vectorTopKOpen, vectorTopKClose,
  vectorTopKFilter, vectorTopKNext, vectorTopKEof, vectorTopKColumn, vectorTopKRowid,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
};

static void functionError(sqlite3_str *errors, const char *zName);

/* generate_series(start, stop, step) and its alias stats_seq, as sqlean's stats extension declares
** them (SQLite's own series.c): each row's value, under its three arguments as hidden columns. Its
** plan takes an equality constraint on each argument the call gives, and is refused without a
** start, as sqlean's is. Only ever prepared here, so a scan of it has no rows. */
static int seriesConnect(sqlite3 *db, void *pAux, int argc, const char *const *argv, sqlite3_vtab **ppVtab, char **pzErr){
  int rc;
  (void)pAux; (void)argc; (void)argv; (void)pzErr;
  rc = sqlite3_declare_vtab(db, "CREATE TABLE x(value,start hidden,stop hidden,step hidden)");
  if( rc!=SQLITE_OK ) return rc;
  *ppVtab = sqlite3_malloc(sizeof(sqlite3_vtab));
  if( *ppVtab==0 ) return SQLITE_NOMEM;
  memset(*ppVtab, 0, sizeof(sqlite3_vtab));
  return SQLITE_OK;
}
static int seriesBestIndex(sqlite3_vtab *pVtab, sqlite3_index_info *pInfo){
  int i, nArg = 0, aIdx[3] = {-1, -1, -1}, bStart = 0, unusable = 0, used = 0;
  for(i=0; i<pInfo->nConstraint; i++){
    const struct sqlite3_index_constraint *p = &pInfo->aConstraint[i];
    int iCol = p->iColumn - 1;
    if( iCol<0 || iCol>2 ) continue;
    if( iCol==0 ) bStart = 1;
    if( !p->usable ){ unusable |= 1<<iCol; continue; }
    if( p->op==SQLITE_INDEX_CONSTRAINT_EQ ){ aIdx[iCol] = i; used |= 1<<iCol; }
  }
  for(i=0; i<3; i++) if( aIdx[i]>=0 ){ pInfo->aConstraintUsage[aIdx[i]].argvIndex = ++nArg; pInfo->aConstraintUsage[aIdx[i]].omit = 1; }
  if( !bStart ){
    sqlite3_free(pVtab->zErrMsg);
    pVtab->zErrMsg = sqlite3_mprintf("first argument to \"generate_series()\" missing or unusable");
    return SQLITE_ERROR;
  }
  if( (unusable & ~used)!=0 ) return SQLITE_CONSTRAINT;
  pInfo->idxNum = used;
  pInfo->estimatedCost = (used & 3)==3 ? 2.0 : 2147483647.0;
  pInfo->estimatedRows = (used & 3)==3 ? 1000 : 2147483647;
  return SQLITE_OK;
}
static sqlite3_module seriesModule = {
  0, 0, seriesConnect, seriesBestIndex, vectorTopKDisconnect, 0, vectorTopKOpen, vectorTopKClose,
  vectorTopKFilter, vectorTopKNext, vectorTopKEof, vectorTopKColumn, vectorTopKRowid,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
};

/* libSQL's functions and sqlean's, vector_top_k, generate_series and stats_seq on the replay, but a
** name this build has its own function of: a scalar function or an aggregate, by its kind. */
static void openLibsql(sqlite3_str *errors){
  enum { N = sizeof(aTurso)/sizeof(aTurso[0]) };
  char aHas[N];
  int i, rc;
  /* Settled before any is registered: a stand-in for one argument count must not hide that this
  ** build lacks the name. */
  for(i=0; i<N; i++) aHas[i] = sqlite3FindFunction(gDb, aTurso[i].zName, -2, SQLITE_UTF8, 0)!=0;
  for(i=0; i<N; i++){
    const struct TursoFunction *f = &aTurso[i];
    if( aHas[i] ) continue;
    if( f->cKind=='a' ) rc = sqlite3_create_function_v2(gDb, f->zName, f->nArg, f->flags, 0, 0, hostFunctionStub, hostFinalStub, 0);
    else rc = sqlite3_create_function_v2(gDb, f->zName, f->nArg, f->flags, 0, hostFunctionStub, 0, 0, 0);
    if( rc!=SQLITE_OK ) functionError(errors, f->zName);
  }
  if( sqlite3_create_module(gDb, "vector_top_k", &vectorTopKModule, 0)!=SQLITE_OK ) functionError(errors, "vector_top_k");
  if( sqlite3_create_module(gDb, "generate_series", &seriesModule, 0)!=SQLITE_OK ) functionError(errors, "generate_series");
  if( sqlite3_create_module(gDb, "stats_seq", &seriesModule, 0)!=SQLITE_OK ) functionError(errors, "stats_seq");
}

/* For vector_top_k('docs_emb', …), the table whose rows its index holds, as its DDL spells the name,
** which "tables" then describes: what the call returns is that table's key. 0 where the index is
** not written as a string or names no index. */
static const char *vectorTopKTable(SrcItem *pItem){
  ExprList *pArgs;
  Index *pIdx;
  if( !pItem->fg.isTabFunc || pItem->zName==0 || sqlite3StrICmp(pItem->zName, "vector_top_k")!=0 ) return 0;
  pArgs = pItem->u1.pFuncArg;
  if( pArgs==0 || pArgs->nExpr<1 || pArgs->a[0].pExpr==0 || pArgs->a[0].pExpr->op!=TK_STRING ) return 0;
  pIdx = sqlite3FindIndex(gDb, pArgs->a[0].pExpr->u.zToken, 0);
  if( pIdx==0 || pIdx->pTable==0 ) return 0;
  tableSeen(pIdx->pTable);
  return pIdx->pTable->zName;
}

static void functionError(sqlite3_str *errors, const char *zName){
  sqlite3_str_appendchar(errors, 1, '{');
  kv_str(errors, "function", zName);
  kv_str(errors, "message", sqlite3_errmsg(gDb));
  closeObj(errors); sqlite3_str_appendchar(errors, 1, ',');
}

/* Whether a replay step that failed is no failure: the statement creates one of SQLite's own
** tables, which sqlite_schema lists like any other. The replay has made sqlite_sequence itself
** with the first AUTOINCREMENT table, and a virtual table's shadow tables with the virtual table;
** any other sqlite_ table (sqlite_stat1, or sqlite_sequence where no AUTOINCREMENT table is
** left) is made here as the plain table it is, which only a writable schema allows. */
static int internalTable(const char *zDdl, const char *zErr){
  static const char zReserved[] = "object name reserved for internal use: ";
  static const char zExists[] = " already exists";
  char *zErr2 = 0;
  int n, ok = 0;
  if( zErr==0 ) return 0;
  if( strncmp(zErr, zReserved, sizeof(zReserved)-1)==0 ){
    /* Defensive mode makes the schema read-only whatever this says; replayAuth stays on. */
    sqlite3_db_config(gDb, SQLITE_DBCONFIG_DEFENSIVE, 0, (int*)0);
    sqlite3_db_config(gDb, SQLITE_DBCONFIG_WRITABLE_SCHEMA, 1, (int*)0);
    ok = sqlite3_exec(gDb, zDdl, 0, 0, &zErr2)==SQLITE_OK;
    sqlite3_db_config(gDb, SQLITE_DBCONFIG_WRITABLE_SCHEMA, 0, (int*)0);
    sqlite3_db_config(gDb, SQLITE_DBCONFIG_DEFENSIVE, 1, (int*)0);
    if( !ok && zErr2 ){
      n = (int)strlen(zErr2);
      ok = n>(int)sizeof(zExists) && strcmp(&zErr2[n-(sizeof(zExists)-1)], zExists)==0;
    }
    sqlite3_free(zErr2);
    return ok;
  }
  n = (int)strlen(zErr);
  if( strncmp(zErr, "table ", 6)==0 && n>6+(int)sizeof(zExists)-1 && strcmp(&zErr[n-(sizeof(zExists)-1)], zExists)==0 ){
    char *zName = sqlite3_mprintf("%.*s", n-6-(int)(sizeof(zExists)-1), &zErr[6]);
    if( zName ){
      Table *pTab;
      sqlite3Dequote(zName);
      pTab = sqlite3FindTable(gDb, zName, "main");
      ok = pTab!=0 && (pTab->tabFlags & TF_Shadow)!=0;
      sqlite3_free(zName);
    }
  }
  return ok;
}

/* What the schema replay may run: anything but an ATTACH, a DETACH (VACUUM INTO attaches the file
** it writes) or a PRAGMA given a value, none of which a schema needs, and which may reach past the
** connection (hard_heap_limit sets the whole process's, temp_store_directory names a directory). A
** PRAGMA that only reads is let through: FTS3 and R*Tree read page_size as they create a table. */
static int replayAuth(void *pArg, int code, const char *z1, const char *z2, const char *z3, const char *zInside){
  (void)pArg; (void)z1; (void)z3; (void)zInside;
  if( code==SQLITE_ATTACH || code==SQLITE_DETACH ) return SQLITE_DENY;
  if( code==SQLITE_PRAGMA && z2!=0 ) return SQLITE_DENY;
  return SQLITE_OK;
}

/* Replays the DDL into a fresh in-memory database; errors are reported, not fatal. The host's
** functions are registered first, so a view, an index or a CHECK that calls one replays too: the
** ones it registers on its connection (azFn), and of the ones its SQLite lists (azHost, with the
** argument count and 's', 'a' or 'w' for a scalar, an aggregate or a window function; nHost is
** negative when it lists none), every one this build has no function of that name for. */
static int openSchema(
  const char *const *azDdl, int nDdl, const char *const *azFn, const int *aFnArg, int nFn,
  const char *const *azHost, const int *aHostArg, const char *zHostType, int nHost, sqlite3_str *errors
){
  sqlite3_str *k = sqlite3_str_new(0);
  char *zKey;
  char *aMissing = 0;
  int i, rc;
  const int flags = SQLITE_UTF8|SQLITE_DETERMINISTIC|SQLITE_INNOCUOUS;
  const int nBefore = sqlite3_str_length(errors);
  for(i=0; i<nFn; i++){ sqlite3_str_appendf(k, "%s/%d", azFn[i], aFnArg[i]); sqlite3_str_appendchar(k, 1, '\2'); }
  for(i=0; i<nHost; i++){ sqlite3_str_appendf(k, "%s/%d/%c", azHost[i], aHostArg[i], zHostType[i]); sqlite3_str_appendchar(k, 1, '\3'); }
  for(i=0; i<nDdl; i++){ sqlite3_str_appendall(k, azDdl[i]); sqlite3_str_appendchar(k, 1, '\1'); }
  zKey = sqlite3_str_finish(k);
  if( zKey==0 ) zKey = sqlite3_mprintf("");
  /* A replay kept from an earlier statement reports what it reported then, so every statement
  ** analyzed against the schema can say which of its steps failed. */
  if( gDb && gDdlKey && strcmp(gDdlKey, zKey)==0 ){
    sqlite3_free(zKey);
    if( gReplayErrors ) sqlite3_str_appendall(errors, gReplayErrors);
    return SQLITE_OK;
  }
  if( gDb ){ sqlite3_close(gDb); gDb = 0; }
  sqlite3_free(gDdlKey); gDdlKey = zKey;
  sqlite3_free(gReplayErrors); gReplayErrors = 0;
  rc = sqlite3_open(":memory:", &gDb);
  if( rc!=SQLITE_OK ) return rc;
  /* The replay runs whatever text the database's sqlite_schema holds, more than one statement to a
  ** row included, and a database file is anyone's to commit, opened by the type check in an editor
  ** and in CI: nothing it runs may reach past this in-memory connection. The connection attaches no
  ** database, ever, since a statement is only prepared on it; while the DDL replays, defensive mode
  ** is on and replayAuth refuses what it refuses. A refused statement ends its row's replay there,
  ** reported in errors as any failed step is ("not authorized"). */
  sqlite3_limit(gDb, SQLITE_LIMIT_ATTACHED, 0);
  installHooks(0);
  openLibsql(errors);
  for(i=0; i<nFn; i++){
    /* Deterministic and innocuous: whatever the host's own flags, the replay accepts every place
    ** the host's schema could already call it from. */
    if( sqlite3_create_function_v2(gDb, azFn[i], aFnArg[i], flags, 0, hostFunctionStub, 0, 0, 0)!=SQLITE_OK ) functionError(errors, azFn[i]);
  }
  /* Which of the host SQLite's functions this build lacks is settled before any stub is made: a
  ** stub for one argument count must not hide that the next row's count is missing too. */
  if( nHost>0 ) aMissing = sqlite3_malloc64((sqlite3_uint64)nHost);
  for(i=0; aMissing && i<nHost; i++) aMissing[i] = sqlite3FindFunction(gDb, azHost[i], -2, SQLITE_UTF8, 0)==0;
  for(i=0; aMissing && i<nHost; i++){
    int nArg = aHostArg[i]<0 ? -1 : aHostArg[i];
    if( !aMissing[i] ) continue;
    if( zHostType[i]=='w' ) rc = sqlite3_create_window_function(gDb, azHost[i], nArg, flags, 0, hostFunctionStub, hostFinalStub, hostFinalStub, hostFunctionStub, 0);
    else if( zHostType[i]=='a' ) rc = sqlite3_create_function_v2(gDb, azHost[i], nArg, flags, 0, 0, hostFunctionStub, hostFinalStub, 0);
    else rc = sqlite3_create_function_v2(gDb, azHost[i], nArg, flags, 0, hostFunctionStub, 0, 0, 0);
    if( rc!=SQLITE_OK ) functionError(errors, azHost[i]);
  }
  sqlite3_free(aMissing);
  sqlite3_db_config(gDb, SQLITE_DBCONFIG_DEFENSIVE, 1, (int*)0);
  sqlite3_set_authorizer(gDb, replayAuth, 0);
  for(i=0; i<nDdl; i++){
    char *zErr = 0;
    int nTry, rcExec = SQLITE_OK;
    /* A CHECK, an index's expression or a generated column that calls a function the host's
    ** connection does not have (an extension the application loads, such as sqlite-jsonschema's
    ** jsonschema_matches) stops CREATE TABLE and CREATE INDEX, though reading the table never
    ** calls it: SQLite resolves those expressions when it creates them. The function is the
    ** application's, so the replay stubs it, as it stubs the host's own, and runs the statement
    ** again. A statement that calls it itself is still refused by functionAuth. */
    static const char zNoFunction[] = "no such function: ";
    for(nTry=0; nTry<16; nTry++){
      rcExec = sqlite3_exec(gDb, azDdl[i], 0, 0, &zErr);
      if( rcExec==SQLITE_OK || zErr==0 || strncmp(zErr, zNoFunction, sizeof(zNoFunction)-1)!=0 ) break;
      if( sqlite3FindFunction(gDb, &zErr[sizeof(zNoFunction)-1], -2, SQLITE_UTF8, 0)!=0
       || sqlite3_create_function_v2(gDb, &zErr[sizeof(zNoFunction)-1], -1, flags, 0, hostFunctionStub, 0, 0, 0)!=SQLITE_OK ) break;
      sqlite3_free(zErr); zErr = 0;
    }
    if( rcExec!=SQLITE_OK && !internalTable(azDdl[i], zErr) ){
      sqlite3_str_appendchar(errors, 1, '{');
      kv_int(errors, "index", i);
      kv_str(errors, "message", zErr);
      closeObj(errors); sqlite3_str_appendchar(errors, 1, ',');
    }
    sqlite3_free(zErr);
  }
  /* The statement is prepared as the application's SQLite prepares it, which writes a virtual
  ** table's shadow table directly unless the application turns defensive mode on itself. */
  sqlite3_set_authorizer(gDb, 0, 0);
  sqlite3_db_config(gDb, SQLITE_DBCONFIG_DEFENSIVE, 0, (int*)0);
  /* A double-quoted word that names no column is a string to SQLite only where the build allows
  ** it. better-sqlite3, node:sqlite, libSQL and Turso build without it (SQLITE_DQS=0) and refuse
  ** `select "hello"`, so a statement is prepared without it too. The replay above keeps it, since
  ** SQLite always reads a schema it loads from a file that way, whatever the build. */
  sqlite3_db_config(gDb, SQLITE_DBCONFIG_DQS_DML, 0, (int*)0);
  sqlite3_db_config(gDb, SQLITE_DBCONFIG_DQS_DDL, 0, (int*)0);
  if( sqlite3_str_length(errors)>nBefore ) gReplayErrors = sqlite3_mprintf("%s", sqlite3_str_value(errors)+nBefore);
  return SQLITE_OK;
}

/* ---- the functions of the host's SQLite ------------------------------------------------------ */

static const char *const *gHost;  /* the names the host's SQLite lists */
static int gnHost;
static const char *const *gExtra; /* the names the host registers on its connection */
static int gnExtra;
static int gnDenied;              /* calls refused while this statement was prepared */
static char *gDeniedView;         /* the view the first of them is in, if it is in one */

/* SQLite asks this of every function a statement calls, the ones in the views it reads included.
** A function this build has and the host's SQLite does not list is refused, and the prepare fails
** at the call. Both lists are pragma_function_list's, which shows no internal function, so one
** of those is never refused. */
static int functionAuth(void *pArg, int code, const char *z1, const char *zFn, const char *z3, const char *zInside){
  FuncDef *pDef;
  int i;
  (void)pArg; (void)z1; (void)z3;
  if( code!=SQLITE_FUNCTION || zFn==0 ) return SQLITE_OK;
  for(i=0; i<gnHost; i++) if( sqlite3StrICmp(gHost[i], zFn)==0 ) return SQLITE_OK;
  for(i=0; i<gnExtra; i++) if( sqlite3StrICmp(gExtra[i], zFn)==0 ) return SQLITE_OK;
  /* libSQL's and sqlean's are refused by no host's list: the SQLite that reads the database file
  ** reads a libSQL file as its own and lists none of them, and the application's drivers decide
  ** whether its SQLite has them (openLibsql). */
  if( isTursoFunction(zFn) ) return SQLITE_OK;
  pDef = sqlite3FindFunction(gDb, zFn, -2, SQLITE_UTF8, 0);
  if( pDef==0 || (pDef->funcFlags & SQLITE_FUNC_INTERNAL)!=0 ) return SQLITE_OK;
  if( gnDenied++==0 && zInside ){
    Table *pTab = sqlite3FindTable(gDb, zInside, 0);
    if( pTab && IsView(pTab) ) gDeniedView = sqlite3_mprintf("%s", zInside);
  }
  return SQLITE_DENY;
}

/* The schema a failed lookup named, where the replay has no schema of that name: for "no such
** table: baz.users", the `baz` of a `baz.users` in the statement, with the reference's offset
** and length. SQLite's own tokenizer finds it, so quoting and comments are read as SQLite reads
** them. 0 when the message is another one or names a schema that exists (main, temp). */
static char *missingSchema(const char *zSql, const char *zMsg, int *piOfst, int *pnLen){
  static const char zPrefix[] = "no such table: ";
  const unsigned char *z = (const unsigned char*)zSql;
  const unsigned char *aTok[3] = {0, 0, 0};
  int aLen[3] = {0, 0, 0};
  int aType[3] = {0, 0, 0};
  if( strncmp(zMsg, zPrefix, sizeof(zPrefix)-1)!=0 ) return 0;
  while( *z ){
    int tk = 0;
    int n = (int)sqlite3GetToken(z, &tk);
    if( n<=0 ) break;
    if( tk!=TK_SPACE && tk!=TK_COMMENT ){
      int name = tk==TK_ID || tk==TK_STRING || (tk<TK_SPACE && sqlite3ParserFallback(tk)==TK_ID);
      aTok[0] = aTok[1]; aLen[0] = aLen[1]; aType[0] = aType[1];
      aTok[1] = aTok[2]; aLen[1] = aLen[2]; aType[1] = aType[2];
      aTok[2] = z; aLen[2] = n; aType[2] = name ? 1 : tk==TK_DOT ? 2 : 0;
      if( aType[0]==1 && aType[1]==2 && aType[2]==1 ){
        char *zDb = sqlite3_mprintf("%.*s", aLen[0], aTok[0]);
        char *zTab = sqlite3_mprintf("%.*s", aLen[2], aTok[2]);
        char *zFull = 0;
        if( zDb && zTab ){
          sqlite3Dequote(zDb); sqlite3Dequote(zTab);
          zFull = sqlite3_mprintf("%s%s.%s", zPrefix, zDb, zTab);
        }
        if( zFull && strcmp(zFull, zMsg)==0 && sqlite3FindDbName(gDb, zDb)<0 ){
          *piOfst = (int)(aTok[0] - (const unsigned char*)zSql);
          *pnLen = (int)(aTok[2] + aLen[2] - aTok[0]);
          sqlite3_free(zTab); sqlite3_free(zFull);
          return zDb;
        }
        sqlite3_free(zDb); sqlite3_free(zTab); sqlite3_free(zFull);
      }
    }
    z += n;
  }
  return 0;
}

/* The version of the tree's format, reylo_sqlite_analyze's first member: TREE_VERSION in
** src/sqlite_tree.rs, which refuses a tree of another. A change to what either side reads or writes
** moves both. */
#define REYLO_TREE_VERSION 2

/* How deep the user's statement may nest: see reylo_sqlite_analyze. */
#define REYLO_SQLITE_EXPR_DEPTH 350

/*
** Analyzes one statement against the schema DDL, with the host's scalar functions (azFn[i] taking
** aFnArg[i] arguments, -1 for any number) registered on the replay, its columns named as zSent,
** the same statement as the application runs it, names them (zSql when it is null), and the functions of the
** host's SQLite (azHost[i] taking aHostArg[i] arguments, of type zHostType[i]; nHost is negative
** when the host did not list them, and every function this build has is then taken to exist).
** Returns a JSON document (free with reylo_sqlite_free): { version, statement, dml, returning,
** tables, columns, params, readonly, error, schemaErrors }.
*/
char *reylo_sqlite_analyze(
  const char *const *azDdl, int nDdl, const char *zSql, const char *zSent, const char *const *azFn, const int *aFnArg, int nFn,
  const char *const *azHost, const int *aHostArg, const char *zHostType, int nHost
){
  sqlite3_str *out = sqlite3_str_new(0);
  sqlite3_str *schemaErrors = sqlite3_str_new(0);
  sqlite3_stmt *pStmt = 0, *pNames = 0;
  int rc, i, n, nDepth;
  sqlite3_str_appendchar(schemaErrors, 1, '[');
  rc = openSchema(azDdl, nDdl, azFn, aFnArg, nFn, azHost, aHostArg, zHostType, nHost, schemaErrors);
  closeArr(schemaErrors);
  sqlite3_str_appendchar(out, 1, '{');
  kv_int(out, "version", REYLO_TREE_VERSION);
  if( rc!=SQLITE_OK ){
    key(out, "error"); sqlite3_str_appendall(out, "{\"message\":\"cannot open the schema replay\",\"offset\":-1},");
    key(out, "schemaErrors"); sqlite3_str_appendall(out, sqlite3_str_value(schemaErrors) ? sqlite3_str_value(schemaErrors) : "[]");
    closeObj(out);
    sqlite3_free(sqlite3_str_finish(schemaErrors));
    return sqlite3_str_finish(out);
  }
  gStmt = sqlite3_str_new(0); gDml = sqlite3_str_new(0); gReturning = sqlite3_str_new(0);
  gCaptured = 0; gInsertSelect = 0; gnTables = 0;
  gHost = azHost; gnHost = nHost; gExtra = azFn; gnExtra = nFn; gnDenied = 0; gDeniedView = 0;
  if( nHost>=0 ) sqlite3_set_authorizer(gDb, functionAuth, 0);
  installHooks(1);
  /* The statement's expressions nest at most REYLO_SQLITE_EXPR_DEPTH deep, 350 where SQLite's own
  ** bound is 1,000: the analysis of a tree that deep is what the WebAssembly engine can afford on
  ** the JavaScript engine's stack beside a host that holds 45% of it, as the bound on Postgres's
  ** tree is for Postgres (core/src/stack.rs). At 500, a nested CASE or function call needed 55% of Node's
  ** stack before V8 optimized the engine and 62% after on Node 24; at 350 the deepest statement
  ** of every shape needs at most 42% (measured on Node 24 and 26, 2026-10-21). The schema's own
  ** statements keep SQLite's bound. */
  nDepth = sqlite3_limit(gDb, SQLITE_LIMIT_EXPR_DEPTH, REYLO_SQLITE_EXPR_DEPTH);
  rc = sqlite3_prepare_v2(gDb, zSql, -1, &pStmt, 0);
  sqlite3_limit(gDb, SQLITE_LIMIT_EXPR_DEPTH, nDepth);
  installHooks(0);
  if( nHost>=0 ) sqlite3_set_authorizer(gDb, 0, 0);
  gInsertSelect = 0;
  if( rc!=SQLITE_OK ){
    static const char zDenied[] = "not authorized to use function: ";
    const char *zMsg = sqlite3_errmsg(gDb);
    int iOfst = sqlite3_error_offset(gDb);
    int nLen = 0;
    char *zSchema = missingSchema(zSql, zMsg, &iOfst, &nLen);
    key(out, "error"); sqlite3_str_appendchar(out, 1, '{');
    if( gnDenied && strncmp(zMsg, zDenied, sizeof(zDenied)-1)==0 ){
      /* The call is in the view's text, not the statement's: its offset would point nowhere. */
      kv_str(out, "missingFunction", &zMsg[sizeof(zDenied)-1]);
      if( gDeniedView ){ kv_str(out, "view", gDeniedView); iOfst = -1; }
    }
    kv_str(out, "message", zMsg);
    kv_int(out, "offset", iOfst);
    if( zSchema ){ kv_str(out, "missingSchema", zSchema); kv_int(out, "length", nLen); }
    closeObj(out); sqlite3_str_appendchar(out, 1, ',');
    sqlite3_free(zSchema);
  }else{
    key(out, "statement"); if( sqlite3_str_length(gStmt) ) sqlite3_str_appendall(out, sqlite3_str_value(gStmt)); else sqlite3_str_appendall(out, "null");
    sqlite3_str_appendchar(out, 1, ',');
    key(out, "dml"); if( sqlite3_str_length(gDml) ) sqlite3_str_appendall(out, sqlite3_str_value(gDml)); else sqlite3_str_appendall(out, "null");
    sqlite3_str_appendchar(out, 1, ',');
    key(out, "returning"); if( sqlite3_str_length(gReturning) ) sqlite3_str_appendall(out, sqlite3_str_value(gReturning)); else sqlite3_str_appendall(out, "null");
    sqlite3_str_appendchar(out, 1, ',');
    n = sqlite3_column_count(pStmt);
    /* zSent is the statement as the application runs it, whose parameters are written differently,
    ** and SQLite names a column with no alias after its text: `select ?1 + 1` is `?1 + 1` where
    ** `select ? + 1` is `? + 1`. Each name is read from that text, which nothing else is. */
    if( zSent ) sqlite3_prepare_v2(gDb, zSent, -1, &pNames, 0);
    if( pNames && sqlite3_column_count(pNames)!=n ){ sqlite3_finalize(pNames); pNames = 0; }
    key(out, "columns"); sqlite3_str_appendchar(out, 1, '[');
    for(i=0; i<n; i++){
      sqlite3_str_appendchar(out, 1, '{');
      kv_str(out, "name", sqlite3_column_name(pNames ? pNames : pStmt, i));
      kv_str(out, "declared", sqlite3_column_decltype(pStmt, i));
      closeObj(out); sqlite3_str_appendchar(out, 1, ',');
    }
    closeArr(out); sqlite3_str_appendchar(out, 1, ',');
    n = sqlite3_bind_parameter_count(pStmt);
    key(out, "params"); sqlite3_str_appendchar(out, 1, '[');
    for(i=1; i<=n; i++){
      sqlite3_str_appendchar(out, 1, '{');
      kv_str(out, "name", sqlite3_bind_parameter_name(pStmt, i));
      closeObj(out); sqlite3_str_appendchar(out, 1, ',');
    }
    closeArr(out); sqlite3_str_appendchar(out, 1, ',');
    kv_bool(out, "readonly", sqlite3_stmt_readonly(pStmt));
    key(out, "tables"); tables(out); sqlite3_str_appendchar(out, 1, ',');
  }
  key(out, "schemaErrors"); sqlite3_str_appendall(out, sqlite3_str_value(schemaErrors) ? sqlite3_str_value(schemaErrors) : "[]");
  sqlite3_str_appendchar(out, 1, ',');
  closeObj(out);
  sqlite3_finalize(pNames);
  sqlite3_finalize(pStmt);
  sqlite3_free(sqlite3_str_finish(gStmt)); sqlite3_free(sqlite3_str_finish(gDml)); sqlite3_free(sqlite3_str_finish(gReturning));
  sqlite3_free(sqlite3_str_finish(schemaErrors));
  gStmt = gDml = gReturning = 0;
  sqlite3_free(gDeniedView); gDeniedView = 0;
  gHost = gExtra = 0; gnHost = gnExtra = 0;
  return sqlite3_str_finish(out);
}

void reylo_sqlite_free(char *p){ sqlite3_free(p); }

/* The version of this SQLite, as "3.53.4": the one that prepares every statement the engine types. */
const char *reylo_sqlite_version(void){ return sqlite3_libversion(); }

#ifdef REYLO_SQLITE_MAIN
/* reylo_sqlite_check <database file> <statements file>: the DDL of the file, one JSON line per statement. */
int main(int argc, char **argv){
  sqlite3 *db; sqlite3_stmt *st; FILE *f; char line[8192];
  const char *azDdl[1024]; int nDdl = 0;
  if( argc<3 ){ fprintf(stderr, "usage: reylo_sqlite_check <db> <statements>\n"); return 2; }
  if( sqlite3_open_v2(argv[1], &db, SQLITE_OPEN_READONLY, 0)!=SQLITE_OK ){ fprintf(stderr, "open: %s\n", sqlite3_errmsg(db)); return 1; }
  sqlite3_prepare_v2(db, "select sql from sqlite_schema where sql is not null order by rowid", -1, &st, 0);
  while( sqlite3_step(st)==SQLITE_ROW && nDdl<1024 ) azDdl[nDdl++] = strdup((const char*)sqlite3_column_text(st, 0));
  sqlite3_finalize(st); sqlite3_close(db);
  f = fopen(argv[2], "r");
  if( f==0 ){ fprintf(stderr, "cannot read %s\n", argv[2]); return 1; }
  while( fgets(line, sizeof line, f) ){
    char *nl = strchr(line, '\n'); if( nl ) *nl = 0;
    if( line[0]==0 || line[0]=='#' ) continue;
    char *json = reylo_sqlite_analyze(azDdl, nDdl, line, 0, 0, 0, 0, 0, 0, 0, -1);
    printf("%s\n", json);
    reylo_sqlite_free(json);
  }
  return 0;
}
#endif
