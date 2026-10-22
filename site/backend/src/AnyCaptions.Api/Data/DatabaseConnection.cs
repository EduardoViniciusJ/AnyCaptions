using Npgsql;
using Microsoft.AspNetCore.WebUtilities;

namespace AnyCaptions.Api.Data;

public static class DatabaseConnection
{
    public static string Resolve(IConfiguration configuration)
    {
        var configured = configuration.GetConnectionString("DefaultConnection");
        if (!string.IsNullOrWhiteSpace(configured)) return configured;
        var value = configuration["DATABASE_URL"];
        if (!Uri.TryCreate(value, UriKind.Absolute, out var uri)
            || uri.Scheme is not ("postgres" or "postgresql"))
            throw new InvalidOperationException("Set ConnectionStrings:DefaultConnection or DATABASE_URL.");
        var userInfo = uri.UserInfo.Split(':', 2);
        var connection = new NpgsqlConnectionStringBuilder
        {
            Host = uri.Host,
            Port = uri.IsDefaultPort ? 5432 : uri.Port,
            Database = Uri.UnescapeDataString(uri.AbsolutePath.TrimStart('/')),
            Username = Uri.UnescapeDataString(userInfo[0]),
            Password = userInfo.Length > 1 ? Uri.UnescapeDataString(userInfo[1]) : ""
        };
        var query = QueryHelpers.ParseQuery(uri.Query);
        if (query.TryGetValue("sslmode", out var sslMode))
        {
            if (!Enum.TryParse<SslMode>(sslMode.ToString().Replace("-", ""), true, out var parsed))
                throw new InvalidOperationException("DATABASE_URL has an unsupported sslmode.");
            connection.SslMode = parsed;
        }
        return connection.ConnectionString;
    }
}
