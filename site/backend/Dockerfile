FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
WORKDIR /source
COPY src/AnyCaptions.Api/*.csproj src/AnyCaptions.Api/
RUN dotnet restore src/AnyCaptions.Api/AnyCaptions.Api.csproj
COPY src/AnyCaptions.Api/ src/AnyCaptions.Api/
RUN dotnet publish src/AnyCaptions.Api/AnyCaptions.Api.csproj -c Release -o /app --no-restore

FROM mcr.microsoft.com/dotnet/aspnet:10.0
WORKDIR /app
COPY --from=build /app .
ENV ASPNETCORE_URLS=http://+:8080
EXPOSE 8080
USER $APP_UID
ENTRYPOINT ["dotnet", "AnyCaptions.Api.dll"]
