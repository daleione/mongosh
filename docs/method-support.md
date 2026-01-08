# MongoDB Shell Commands Support Status

This document tracks the implementation status of MongoDB Shell commands in mongosh.

## Legend

- ✅ **Supported** - Fully implemented and tested
- ⚠️ **Partial** - Partially implemented or has known limitations
- ❌ **Not Supported** - Not yet implemented
- 🚫 **Deprecated** - Deprecated by MongoDB, will not be implemented
- 📝 **Planned** - Scheduled for future implementation
- 🔍 **Under Review** - Currently being evaluated

---

## Administration Methods

| Method                | Status | Notes                                           |
| --------------------- | ------ | ----------------------------------------------- |
| `db.adminCommand()`   | ❌     | Runs a command against the `admin` database     |
| `db.currentOp()`      | ❌     | Reports the current in-progress operations      |
| `db.killOp()`         | ❌     | Terminates a specified operation                |
| `db.shutdownServer()` | ❌     | Shuts down the current mongod or mongos process |
| `db.fsyncLock()`      | ❌     | Flushes writes to disk and locks the database   |
| `db.fsyncUnlock()`    | ❌     | Allows writes to continue on a locked database  |

---

## Atlas Search Index Methods

| Method                              | Status | Notes                                                   |
| ----------------------------------- | ------ | ------------------------------------------------------- |
| `db.collection.createSearchIndex()` | ❌     | Creates one or more Atlas Search indexes                |
| `db.collection.dropSearchIndex()`   | ❌     | Deletes an existing Atlas Search index                  |
| `db.collection.getSearchIndexes()`  | ❌     | Returns information about existing Atlas Search indexes |
| `db.collection.updateSearchIndex()` | ❌     | Updates an existing Atlas Search index                  |

---

## Bulk Operation Methods

| Method                                      | Status | Notes                                           |
| ------------------------------------------- | ------ | ----------------------------------------------- |
| `db.collection.initializeOrderedBulkOp()`   | ❌     | Initializes ordered bulk operations builder     |
| `db.collection.initializeUnorderedBulkOp()` | ❌     | Initializes unordered bulk operations builder   |
| `Bulk()`                                    | ❌     | Creates a bulk operations builder               |
| `Bulk.execute()`                            | ❌     | Executes the list of bulk operations            |
| `Bulk.find()`                               | ❌     | Specifies a query condition for update/remove   |
| `Bulk.find.hint()`                          | ❌     | Sets the hint option for bulk operation         |
| `Bulk.find.remove()`                        | ❌     | Adds a remove operation to bulk list            |
| `Bulk.find.removeOne()`                     | ❌     | Adds a single document remove operation         |
| `Bulk.find.replaceOne()`                    | ❌     | Adds a single document replacement operation    |
| `Bulk.find.updateOne()`                     | ❌     | Adds a single document update operation         |
| `Bulk.find.update()`                        | ❌     | Adds a multi update operation                   |
| `Bulk.find.upsert()`                        | ❌     | Sets the upsert option to true                  |
| `Bulk.getOperations()`                      | ❌     | Returns array of executed write operations      |
| `Bulk.insert()`                             | ❌     | Adds an insert operation to bulk list           |
| `Bulk.toJSON()`                             | ❌     | Returns JSON document of operations and batches |
| `Bulk.toString()`                           | ❌     | Returns string of JSON document                 |

---

## Collection Methods

| Method                                   | Status | Notes                                           |
| ---------------------------------------- | ------ | ----------------------------------------------- |
| `db.collection.aggregate()`              | ✅     | Provides access to aggregation pipeline         |
| `db.collection.bulkWrite()`              | ❌     | Provides bulk write operation functionality     |
| `db.collection.count()`                  | ✅     | Returns count of documents in collection        |
| `db.collection.countDocuments()`         | ✅     | Returns count of documents in collection        |
| `db.collection.estimatedDocumentCount()` | ❌     | Returns approximate count of documents          |
| `db.collection.createIndex()`            | ✅     | Builds an index on a collection                 |
| `db.collection.createIndexes()`          | ✅     | Builds one or more indexes on a collection      |
| `db.collection.dataSize()`               | ❌     | Returns the size of the collection              |
| `db.collection.deleteOne()`              | ✅     | Deletes a single document                       |
| `db.collection.deleteMany()`             | ✅     | Deletes multiple documents                      |
| `db.collection.distinct()`               | ❌     | Returns array of distinct values                |
| `db.collection.drop()`                   | ❌     | Removes the specified collection                |
| `db.collection.dropIndex()`              | ❌     | Removes a specified index                       |
| `db.collection.dropIndexes()`            | ❌     | Removes all indexes on a collection             |
| `db.collection.ensureIndex()`            | 🚫     | **Deprecated** - Use createIndex                |
| `db.collection.explain()`                | ❌     | Returns query execution information             |
| `db.collection.find()`                   | ✅     | Performs a query and returns cursor             |
| `db.collection.findAndModify()`          | ❌     | Atomically modifies and returns single document |
| `db.collection.findOne()`                | ✅     | Performs a query and returns single document    |
| `db.collection.findOneAndDelete()`       | ❌     | Finds and deletes a single document             |
| `db.collection.findOneAndReplace()`      | ❌     | Finds and replaces a single document            |
| `db.collection.findOneAndUpdate()`       | ❌     | Finds and updates a single document             |
| `db.collection.getIndexes()`             | ✅     | Returns array of existing indexes               |
| `db.collection.getShardDistribution()`   | ❌     | Prints data distribution for sharded collection |
| `db.collection.getShardVersion()`        | ❌     | Returns state of data in sharded cluster        |
| `db.collection.insertOne()`              | ✅     | Inserts a new document                          |
| `db.collection.insertMany()`             | ✅     | Inserts several new documents                   |
| `db.collection.isCapped()`               | ❌     | Reports if collection is capped                 |
| `db.collection.mapReduce()`              | 🚫     | Use aggregation pipeline instead                |
| `db.collection.reIndex()`                | ❌     | Rebuilds all existing indexes                   |
| `db.collection.renameCollection()`       | ❌     | Changes the name of a collection                |
| `db.collection.replaceOne()`             | ❌     | Replaces a single document                      |
| `db.collection.stats()`                  | ❌     | Reports on the state of a collection            |
| `db.collection.storageSize()`            | ❌     | Reports total size used by collection           |
| `db.collection.totalIndexSize()`         | ❌     | Reports total size used by indexes              |
| `db.collection.totalSize()`              | ❌     | Reports total size of collection                |
| `db.collection.updateOne()`              | ✅     | Modifies a single document                      |
| `db.collection.updateMany()`             | ✅     | Modifies multiple documents                     |
| `db.collection.validate()`               | ❌     | Validates a collection                          |
| `db.collection.watch()`                  | ❌     | Opens a change stream cursor                    |

---

## Connection Methods

| Method                | Status | Notes                                                      |
| --------------------- | ------ | ---------------------------------------------------------- |
| `Mongo()`             | ❌     | JavaScript constructor to instantiate database connection  |
| `Mongo.getDB()`       | ❌     | Returns a database object                                  |
| `Mongo.setReadPref()` | ❌     | Sets the read preference for connection                    |
| `Mongo.watch()`       | ❌     | Opens change stream cursor for replica set/sharded cluster |

---

## Cursor Methods

| Method                     | Status | Notes                                                                          |
| -------------------------- | ------ | ------------------------------------------------------------------------------ |
| `cursor.addOption()`       | ❌     | Adds special wire protocol flags                                               |
| `cursor.batchSize()`       | ⚠️     | Specifies maximum documents per batch (parsed but not fully applied)           |
| `cursor.close()`           | ❌     | Closes cursor and frees server resources                                       |
| `cursor.collation()`       | ⚠️     | Specifies the collation for cursor (parsed but not fully applied)              |
| `cursor.comment()`         | ❌     | Attaches a comment to the query                                                |
| `cursor.count()`           | 🚫     | **Deprecated** - Use countDocuments() instead                                  |
| `cursor.explain()`         | ❌     | Reports on query execution plan                                                |
| `cursor.forEach()`         | ❌     | Applies JavaScript function for every document                                 |
| `cursor.hasNext()`         | ❌     | Returns true if cursor has documents                                           |
| `cursor.hint()`            | ⚠️     | Forces MongoDB to use specific index (parsed but not fully applied)            |
| `cursor.isClosed()`        | ❌     | Returns true if cursor is closed                                               |
| `cursor.isExhausted()`     | ❌     | Returns true if cursor is closed and no objects remaining                      |
| `cursor.itcount()`         | ❌     | Computes total number of documents client-side                                 |
| `cursor.limit()`           | ✅     | Constrains the size of cursor's result set                                     |
| `cursor.map()`             | ❌     | Applies function and collects return values                                    |
| `cursor.max()`             | ❌     | Specifies exclusive upper index bound                                          |
| `cursor.maxTimeMS()`       | ⚠️     | Specifies cumulative time limit in milliseconds (parsed but not fully applied) |
| `cursor.min()`             | ❌     | Specifies inclusive lower index bound                                          |
| `cursor.next()`            | ❌     | Returns the next document in cursor                                            |
| `cursor.noCursorTimeout()` | ❌     | Instructs server to avoid closing cursor automatically                         |
| `cursor.objsLeftInBatch()` | ❌     | Returns number of documents left in current batch                              |
| `cursor.readConcern()`     | ⚠️     | Specifies a read concern (parsed but not fully applied)                        |
| `cursor.readPref()`        | ❌     | Specifies a read preference                                                    |
| `cursor.returnKey()`       | ❌     | Modifies cursor to return index keys                                           |
| `cursor.showRecordId()`    | ❌     | Adds internal storage engine ID field                                          |
| `cursor.size()`            | ❌     | Returns count after applying skip and limit                                    |
| `cursor.skip()`            | ✅     | Returns cursor skipping specified number of documents                          |
| `cursor.sort()`            | ✅     | Returns results ordered by sort specification                                  |
| `cursor.tailable()`        | ❌     | Marks cursor as tailable                                                       |
| `cursor.toArray()`         | ❌     | Returns array of all documents returned by cursor                              |

---

## Database Methods

| Method                     | Status | Notes                                             |
| -------------------------- | ------ | ------------------------------------------------- |
| `db.aggregate()`           | ❌     | Runs admin/diagnostic pipeline                    |
| `db.createCollection()`    | ❌     | Creates a new collection or view                  |
| `db.createView()`          | ❌     | Creates a view from aggregation pipeline          |
| `db.commandHelp()`         | ❌     | Displays help text for database command           |
| `db.dropDatabase()`        | ❌     | Removes the current database                      |
| `db.getCollection()`       | ❌     | Returns a collection or view object               |
| `db.getCollectionInfos()`  | ❌     | Returns collection information                    |
| `db.getCollectionNames()`  | ❌     | Lists all collections and views                   |
| `db.getMongo()`            | ❌     | Returns the current database connection           |
| `db.getLogComponents()`    | ❌     | Returns current log verbosity settings            |
| `db.getName()`             | ❌     | Returns the name of current database              |
| `db.getProfilingStatus()`  | ❌     | Returns current profile level and settings        |
| `db.getSiblingDB()`        | ❌     | Provides access to specified database             |
| `db.listCommands()`        | ❌     | Provides list of all database commands            |
| `db.logout()`              | ❌     | Ends an authenticated session                     |
| `db.printShardingStatus()` | ❌     | Prints formatted report of sharding configuration |
| `db.runCommand()`          | ❌     | Runs a database command                           |
| `db.setLogLevel()`         | ❌     | Sets a single verbosity level for log messages    |
| `db.setProfilingLevel()`   | ❌     | Configures database profiler level                |
| `db.watch()`               | ❌     | Opens change stream cursor for database           |

---

## In-Use Encryption Methods

| Method                                         | Status | Notes                                       |
| ---------------------------------------------- | ------ | ------------------------------------------- |
| `ClientEncryption.createEncryptedCollection()` | ❌     | Creates collection with encrypted fields    |
| `ClientEncryption.decrypt()`                   | ❌     | Decrypts the specified encrypted value      |
| `ClientEncryption.encrypt()`                   | ❌     | Encrypts the specified value                |
| `getClientEncryption()`                        | ❌     | Returns ClientEncryption object             |
| `getKeyVault()`                                | ❌     | Returns KeyVault object                     |
| `KeyVault.addKeyAlternateName()`               | ❌     | Adds keyAltName to data encryption key      |
| `KeyVault.createKey()`                         | ❌     | Adds data encryption key to key vault       |
| `KeyVault.deleteKey()`                         | ❌     | Deletes data encryption key                 |
| `KeyVault.getKey()`                            | ❌     | Gets data encryption key by UUID            |
| `KeyVault.getKeyByAltName()`                   | ❌     | Gets data encryption keys by alternate name |
| `KeyVault.getKeys()`                           | ❌     | Returns all data encryption keys            |
| `KeyVault.removeKeyAlternateName()`            | ❌     | Removes keyAltName from data encryption key |

---

## Native Methods

| Method            | Status | Notes                                          |
| ----------------- | ------ | ---------------------------------------------- |
| `buildInfo()`     | ❌     | Returns mongosh build and driver dependencies  |
| `isInteractive()` | ❌     | Returns boolean for interactive vs script mode |
| `load()`          | ❌     | Loads and runs JavaScript file in shell        |
| `print()`         | ❌     | Prints specified text or variable              |
| `quit()`          | ✅     | Exits the current shell session                |
| `sleep()`         | ❌     | Suspends shell for given period                |
| `version()`       | ❌     | Returns current mongosh version                |

---

## Query Plan Cache Methods

| Method                          | Status | Notes                                     |
| ------------------------------- | ------ | ----------------------------------------- |
| `db.collection.getPlanCache()`  | ❌     | Returns interface to query plan cache     |
| `PlanCache.clear()`             | ❌     | Removes all cached query plans            |
| `PlanCache.clearPlansByQuery()` | ❌     | Clears cached query plans for query shape |
| `PlanCache.help()`              | ❌     | Lists methods to view/modify plan cache   |
| `PlanCache.list()`              | ❌     | Returns array of plan cache entries       |

---

## Replication Methods

| Method                               | Status | Notes                                      |
| ------------------------------------ | ------ | ------------------------------------------ |
| `rs.add()`                           | ❌     | Adds a member to replica set               |
| `rs.addArb()`                        | ❌     | Adds an arbiter to replica set             |
| `rs.config()`                        | ❌     | Returns current replica set configuration  |
| `rs.freeze()`                        | ❌     | Makes member ineligible to become primary  |
| `db.getReplicationInfo()`            | ❌     | Returns replica set status from oplog data |
| `rs.initiate()`                      | ❌     | Initializes a new replica set              |
| `db.printReplicationInfo()`          | ❌     | Returns oplog of replica set member        |
| `rs.printReplicationInfo()`          | ❌     | Returns oplog of replica set member        |
| `db.printSecondaryReplicationInfo()` | ❌     | Returns status of secondary members        |
| `rs.printSecondaryReplicationInfo()` | ❌     | Returns status of secondary members        |
| `rs.reconfig()`                      | ❌     | Modifies replica set configuration         |
| `rs.remove()`                        | ❌     | Removes member from replica set            |
| `rs.status()`                        | ❌     | Returns replica set member status          |
| `rs.stepDown()`                      | ❌     | Makes primary become secondary             |
| `rs.syncFrom()`                      | ❌     | Resets sync target for replica set member  |

---

## Role Management Methods

| Method                          | Status | Notes                                          |
| ------------------------------- | ------ | ---------------------------------------------- |
| `db.createRole()`               | ❌     | Creates a role and specifies privileges        |
| `db.dropRole()`                 | ❌     | Deletes a user-defined role                    |
| `db.dropAllRoles()`             | ❌     | Deletes all user-defined roles                 |
| `db.getRole()`                  | ❌     | Returns information for specified role         |
| `db.getRoles()`                 | ❌     | Returns information for all user-defined roles |
| `db.grantPrivilegesToRole()`    | ❌     | Assigns privileges to user-defined role        |
| `db.revokePrivilegesFromRole()` | ❌     | Removes privileges from user-defined role      |
| `db.grantRolesToRole()`         | ❌     | Specifies roles from which role inherits       |
| `db.revokeRolesFromRole()`      | ❌     | Removes inherited roles from role              |
| `db.updateRole()`               | ❌     | Updates a user-defined role                    |

---

## Session Object Methods

| Method                           | Status | Notes                                            |
| -------------------------------- | ------ | ------------------------------------------------ |
| `Mongo.startSession()`           | ❌     | Starts a session for connection                  |
| `Session.advanceOperationTime()` | ❌     | Updates the operation time                       |
| `Session.endSession()`           | ❌     | Ends the session                                 |
| `Session.getClusterTime()`       | ❌     | Returns most recent cluster time                 |
| `Session.getDatabase()`          | ❌     | Access database from session                     |
| `Session.getOperationTime()`     | ❌     | Returns timestamp of last acknowledged operation |
| `Session.getOptions()`           | ❌     | Returns options for session                      |
| `Session.hasEnded()`             | ❌     | Returns boolean if session has ended             |
| `SessionOptions()`               | ❌     | The options for a session                        |

---

## Server Status Methods

| Method                         | Status | Notes                                            |
| ------------------------------ | ------ | ------------------------------------------------ |
| `db.hello()`                   | ❌     | Returns document describing mongod instance role |
| `db.hostInfo()`                | ❌     | Returns document with system information         |
| `db.collection.latencyStats()` | ❌     | Returns latency statistics for collection        |
| `db.printCollectionStats()`    | ❌     | Returns statistics from every collection         |
| `db.serverBuildInfo()`         | ❌     | Returns compilation parameters for mongod        |
| `db.serverCmdLineOpts()`       | ❌     | Returns runtime options information              |
| `db.serverStatus()`            | ❌     | Returns overview of database process             |
| `db.stats()`                   | ❌     | Reports on state of current database             |
| `db.version()`                 | ❌     | Returns version of mongod instance               |

---

## Sharding Methods

| Method                             | Status | Notes                                             |
| ---------------------------------- | ------ | ------------------------------------------------- |
| `db.collection.getShardLocation()` | ❌     | Returns shards where collection is located        |
| `sh.addShard()`                    | ❌     | Adds a shard to sharded cluster                   |
| `sh.addShardTag()`                 | ❌     | Aliases to sh.addShardToZone()                    |
| `sh.addShardToZone()`              | ❌     | Associates shard with zone                        |
| `sh.addTagRange()`                 | ❌     | Aliases to sh.updateZoneKeyRange()                |
| `sh.balancerCollectionStatus()`    | ❌     | Returns chunk balance information                 |
| `sh.disableAutoMerger()`           | ❌     | Disables automatic chunk merges                   |
| `sh.disableAutoSplit()`            | ❌     | Disables auto-splitting for cluster               |
| `sh.disableBalancing()`            | ❌     | Disables balancing on single collection           |
| `sh.disableMigrations()`           | ❌     | Disables chunk migrations for collection          |
| `sh.enableAutoMerger()`            | ❌     | Enables automatic chunk merges                    |
| `sh.enableAutoSplit()`             | ❌     | Enables auto-splitting for cluster                |
| `sh.enableBalancing()`             | ❌     | Activates sharded collection balancer             |
| `sh.enableMigrations()`            | ❌     | Enables chunk migrations for collection           |
| `sh.enableSharding()`              | ❌     | Enables sharding on specific database             |
| `sh.getBalancerState()`            | ❌     | Returns boolean if balancer is enabled            |
| `sh.getShardedDataDistribution()`  | ❌     | Returns data distribution for sharded collections |
| `sh.isBalancerRunning()`           | ❌     | Returns boolean if balancer is migrating chunks   |
| `sh.isConfigShardEnabled()`        | ❌     | Returns whether cluster has config shard          |
| `sh.listShards()`                  | ❌     | Returns array of documents describing shards      |
| `sh.moveChunk()`                   | ❌     | Migrates a chunk in sharded cluster               |
| `sh.removeRangeFromZone()`         | ❌     | Removes association between range and zone        |
| `sh.removeShardFromZone()`         | ❌     | Removes association between shard and zone        |
| `sh.removeShardTag()`              | ❌     | Removes association between tag and shard         |
| `sh.removeTagRange()`              | ❌     | Removes range of shard key values                 |
| `sh.setBalancerState()`            | ❌     | Enables or disables the balancer                  |
| `sh.shardCollection()`             | ❌     | Enables sharding for collection                   |
| `sh.splitAt()`                     | ❌     | Divides chunk using specific shard key value      |
| `sh.splitFind()`                   | ❌     | Divides chunk containing document matching query  |
| `sh.startAutoMerger()`             | ❌     | Enables the AutoMerger                            |
| `sh.startBalancer()`               | ❌     | Enables the balancer                              |
| `sh.status()`                      | ❌     | Reports on status of sharded cluster              |
| `sh.stopAutoMerger()`              | ❌     | Disables the AutoMerger                           |
| `sh.stopBalancer()`                | ❌     | Disables the balancer                             |
| `sh.updateZoneKeyRange()`          | ❌     | Associates range of shard keys with zone          |

---

## Telemetry Methods

| Method               | Status | Notes                         |
| -------------------- | ------ | ----------------------------- |
| `disableTelemetry()` | ❌     | Disable telemetry for mongosh |
| `enableTelemetry()`  | ❌     | Enable telemetry for mongosh  |

---

## Transaction Methods

| Method                        | Status | Notes                                 |
| ----------------------------- | ------ | ------------------------------------- |
| `Session.abortTransaction()`  | ❌     | Terminates multi-document transaction |
| `Session.commitTransaction()` | ❌     | Saves changes and ends transaction    |
| `Session.startTransaction()`  | ❌     | Starts multi-document transaction     |

---

## User Management Methods

| Method                     | Status | Notes                                      |
| -------------------------- | ------ | ------------------------------------------ |
| `db.auth()`                | ❌     | Authenticates a user to database           |
| `db.changeUserPassword()`  | ❌     | Changes existing user's password           |
| `db.createUser()`          | ❌     | Creates a new user                         |
| `db.dropAllUsers()`        | ❌     | Deletes all users associated with database |
| `db.dropUser()`            | ❌     | Deletes a single user                      |
| `db.getUser()`             | ❌     | Returns information about specified user   |
| `db.getUsers()`            | ❌     | Returns information about all users        |
| `db.updateUser()`          | ❌     | Updates specified user's data              |
| `db.grantRolesToUser()`    | ❌     | Grants role and privileges to user         |
| `db.revokeRolesFromUser()` | ❌     | Removes role from user                     |
