using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace AnyCaptions.Api.Data;

public sealed class AppDbContextFactory : IDesignTimeDbContextFactory<AppDbContext>
{
    public AppDbContext CreateDbContext(string[] args)
    {
        var configuration = new ConfigurationBuilder().AddEnvironmentVariables().Build();
        var options = new DbContextOptionsBuilder<AppDbContext>()
            .UseNpgsql(DatabaseConnection.Resolve(configuration)).Options;
        return new AppDbContext(options);
    }
}
