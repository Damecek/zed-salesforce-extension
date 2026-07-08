public class InlineSoqlFormulaKnownGap {
  public static List<Order__c> highProfitOrders() {
    return [
      SELECT Id, Name
      FROM Order__c
      WHERE FORMULA('Revenue__c - Cost__c') > 250
    ];
  }
}
