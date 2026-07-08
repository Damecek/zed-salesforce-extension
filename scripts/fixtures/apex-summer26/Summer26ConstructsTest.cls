@IntegrationTest
public with sharing class Summer26ConstructsTest {
  @IntegrationTest
  public static void multilineStringTemplateAndUserModeDml() {
    String body = '''
{
  "name": "${name}",
  "status": "ready"
}
''';
    String rendered = String.template(body, new Map<String, Object>{ 'name' => 'Acme' });
    Account accountRecord = new Account(Name = rendered);
    insert as user accountRecord;
    IntegrationTest.commitTestOnly();
    System.debug(rendered);
  }

  @TearDown
  public static void cleanup() {
    delete as system [SELECT Id FROM Account WHERE Name LIKE 'Acme%'];
  }
}
