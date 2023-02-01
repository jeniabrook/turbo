window.SIDEBAR_ITEMS = {"struct":[["ManifestChunkAsset","The manifest chunk is deferred until requested by the manifest loader item when the dynamic `import()` expression is reached. Its responsibility is to generate a Promise that will resolve only after all the necessary chunks needed by the dynamic import are loaded by the client."],["ManifestChunkAssetVc","Vc for [`ManifestChunkAsset`]"],["ManifestLoaderItem","The manifest loader item is shipped in the same chunk that uses the dynamic `import()` expression. Its responsibility is to load the manifest chunk from the server. The dynamic import has been rewritten to import this manifest loader item, which will load the manifest chunk from the server, which will load all the chunks needed by the dynamic import. Finally, we’ll be able to import the module we’re trying to dynamically import."],["ManifestLoaderItemVc","Vc for [`ManifestLoaderItem`]"]],"type":[["ManifestChunkAssetReadRef","see [turbo_tasks::ReadRef]"],["ManifestLoaderItemReadRef","see [turbo_tasks::ReadRef]"]]};