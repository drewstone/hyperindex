const postgres = require("postgres")

const postgresConfig = require("./Config.bs.js").db

const sql = postgres({...postgresConfig, 
  transform: {
    undefined: null
  }
})

  // db operations for User:

  module.exports.readUserEntities = (entityIdArray) => sql`
  SELECT *
  FROM public.user
  WHERE id IN ${sql(entityIdArray)}`

  module.exports.batchSetUser = (entityDataArray) => {
  const valueCopyToFixBigIntType = entityDataArray // This is required for BigInts to work in the db. See: https://github.com/Float-Capital/indexer/issues/212
  return sql`
    INSERT INTO public.user
  ${sql(valueCopyToFixBigIntType,
    "id",
    "address",
    "gravatar",
  )}
    ON CONFLICT(id) DO UPDATE
    SET
    "id" = EXCLUDED."id"
      ,
    "address" = EXCLUDED."address"
      ,
    "gravatar" = EXCLUDED."gravatar"
  ;`
  }

  module.exports.batchDeleteUser = (entityIdArray) => sql`
  DELETE
  FROM public.user
  WHERE id IN ${sql(entityIdArray)};`
  // end db operations for User
  // db operations for Gravatar:

  module.exports.readGravatarEntities = (entityIdArray) => sql`
  SELECT *
  FROM public.gravatar
  WHERE id IN ${sql(entityIdArray)}`

  module.exports.batchSetGravatar = (entityDataArray) => {
  const valueCopyToFixBigIntType = entityDataArray // This is required for BigInts to work in the db. See: https://github.com/Float-Capital/indexer/issues/212
  return sql`
    INSERT INTO public.gravatar
  ${sql(valueCopyToFixBigIntType,
    "id",
    "owner",
    "displayName",
    "imageUrl",
    "updatesCount",
  )}
    ON CONFLICT(id) DO UPDATE
    SET
    "id" = EXCLUDED."id"
      ,
    "owner" = EXCLUDED."owner"
      ,
    "displayName" = EXCLUDED."displayName"
      ,
    "imageUrl" = EXCLUDED."imageUrl"
      ,
    "updatesCount" = EXCLUDED."updatesCount"
  ;`
  }

  module.exports.batchDeleteGravatar = (entityIdArray) => sql`
  DELETE
  FROM public.gravatar
  WHERE id IN ${sql(entityIdArray)};`
  // end db operations for Gravatar