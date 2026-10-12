/* Add DSL highlighting using mdBook's bundled highlight.js API. */
if (window.hljs) {
    hljs.registerLanguage('dctx0', function (hljs) {
        return {
            name: 'DCTX0',
            keywords: {
                keyword: 'workspace domain fragment module external surface port operation intent scalar value type entity message event failure owns requires effects read write call emit observe state by from lists observes inserts upserts replaces removes range length format where key stableId example given returns when expect fails emits invariant forall assert pure ensures lifecycle transition for using initial terminal to decision choose because avoid constraint proof test review inspect criteria question policy require',
                literal: 'true false None Some Success PASS FAIL UNKNOWN',
                type: 'String Boolean Int Float Decimal UUID Timestamp Duration Bytes Unit Option List'
            },
            contains: [hljs.C_LINE_COMMENT_MODE, hljs.C_BLOCK_COMMENT_MODE,
                {className: 'string', begin: '"""', end: '"""'}, hljs.QUOTE_STRING_MODE,
                hljs.NUMBER_MODE]
        };
    });
    document.querySelectorAll('code.language-dctx0').forEach(function (block) {
        if (block.dataset.highlighted) delete block.dataset.highlighted;
        hljs.highlightBlock(block);
    });
}
